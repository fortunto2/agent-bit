//! Tool completion hooks — data-driven workflow guidance.
//!
//! After any tool executes, hooks check if the action matches a pattern
//! and return messages to inject before the next LLM decision.
//!
//! Hook sources (in priority order):
//! 1. Parsed from AGENTS.MD at trial start (project-specific)
//! 2. Static fallbacks (known PAC1 patterns)
//!
//! Used by:
//! - WriteTool: augments own output (immediate context)
//! - SgrAgent::after_execute: injects session messages (next LLM call)
//! - agent_loop: could inject via nudge system (two-phase mode)

use std::sync::Arc;

/// A single hook: trigger pattern → message to inject.
#[derive(Clone, Debug)]
pub struct ToolHook {
    /// Which tool triggers this hook ("write", "read", "delete", "*" for any)
    pub tool: String,
    /// Path substring to match (lowercase)
    pub path_contains: String,
    /// Paths to exclude from matching
    pub exclude: Vec<String>,
    /// Message to inject after tool completes
    pub message: String,
    /// Where this hook came from
    pub source: HookSource,
}

#[derive(Clone, Debug)]
pub enum HookSource {
    /// Parsed from AGENTS.MD at runtime
    AgentsMd,
    /// Hardcoded fallback
    Static,
}

/// Registry of tool hooks — shared across tools and agent.
#[derive(Clone, Debug, Default)]
pub struct HookRegistry {
    hooks: Vec<ToolHook>,
}

impl HookRegistry {
    pub fn new() -> Self {
        Self { hooks: Vec::new() }
    }

    /// Parse AGENTS.MD content to extract workflow hooks.
    ///
    /// Recognizes patterns like:
    /// - "When adding a card under /02_distill/cards/, also update threads"
    /// - "After writing to X, do Y"
    pub fn from_agents_md(content: &str) -> Self {
        let mut registry = Self::new();

        for line in content.lines() {
            let ll = line.to_lowercase();

            // Pattern: "when adding/writing to {path}, also {action}"
            if (ll.contains("when adding") || ll.contains("when writing") || ll.contains("after writing"))
                && ll.contains("also")
            {
                if let (Some(source_path), Some(action_part)) = (
                    extract_path_ref(line),
                    line.to_lowercase().split("also").nth(1).map(|s| s.to_string()),
                ) {
                    let target_path = extract_path_ref(&action_part);
                    let next_step = action_part.trim().trim_start_matches(',').trim();

                    registry.hooks.push(ToolHook {
                        tool: "write".into(),
                        path_contains: source_path.trim_matches('/').to_lowercase(),
                        exclude: vec!["template".into()],
                        message: format!("NEXT: {} (from AGENTS.MD)", next_step),
                        source: HookSource::AgentsMd,
                    });

                    if let Some(ref target) = target_path {
                        eprintln!("  📎 Hook: write to {} → {}", source_path, target);
                    }
                }
            }

            // Pattern: "keep existing files in {path} immutable"
            if ll.contains("immutable") || ll.contains("do not modify") {
                if let Some(path) = extract_path_ref(line) {
                    registry.hooks.push(ToolHook {
                        tool: "write".into(),
                        path_contains: path.trim_matches('/').to_lowercase(),
                        exclude: vec![],
                        message: format!("⚠ WARNING: files in {} are immutable per AGENTS.MD. Do NOT overwrite.", path),
                        source: HookSource::AgentsMd,
                    });
                    eprintln!("  📎 Hook: write to {} → immutable warning", path);
                }
            }
        }

        // Fallback: if no hooks parsed but content mentions cards + threads
        if registry.hooks.is_empty() {
            let lower = content.to_lowercase();
            if lower.contains("card") && lower.contains("thread") && lower.contains("distill") {
                registry.hooks.push(ToolHook {
                    tool: "write".into(),
                    path_contains: "distill/cards/".into(),
                    exclude: vec!["template".into()],
                    message: "NEXT: update matching thread in 02_distill/threads/ (append card link).".into(),
                    source: HookSource::Static,
                });
                eprintln!("  📎 Hook (fallback): distill/cards → threads");
            }
        }

        registry
    }

    /// Check if a tool action matches any hook. Returns messages to inject.
    pub fn check(&self, tool_name: &str, path: &str) -> Vec<String> {
        let norm = path.trim_start_matches('/').to_lowercase();
        let mut messages = Vec::new();

        for hook in &self.hooks {
            if (hook.tool == tool_name || hook.tool == "*")
                && norm.contains(&hook.path_contains)
                && !hook.exclude.iter().any(|ex| norm.contains(ex))
            {
                messages.push(format!("📌 {}", hook.message));
            }
        }

        messages
    }

    /// Number of registered hooks.
    pub fn len(&self) -> usize {
        self.hooks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.hooks.is_empty()
    }
}

/// Extract first path-like reference from text (markdown link or /path/).
fn extract_path_ref(text: &str) -> Option<String> {
    // Markdown link: [text](/path/)
    if let Some(start) = text.find("](/") {
        if let Some(end) = text[start + 2..].find(')') {
            return Some(text[start + 2..start + 2 + end].to_string());
        }
    }
    // Plain path: /something/something/ or something/something
    for word in text.split_whitespace() {
        let clean = word.trim_matches(|c: char| {
            !c.is_alphanumeric() && c != '/' && c != '_' && c != '-' && c != '.'
        });
        if clean.contains('/') && clean.len() > 3 && !clean.starts_with("http") {
            return Some(clean.to_string());
        }
    }
    None
}

/// Shared hook registry (Arc for multi-tool access).
pub type SharedHookRegistry = Arc<HookRegistry>;

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_AGENTS_MD: &str = "\
Be pragmatic. Prefer small diffs.
Keep existing files in `01_capture/` immutable.
When adding a card under [/02_distill/cards/](/02_distill/cards/), also update 1-2 relevant threads under [/02_distill/threads/](/02_distill/threads/).
";

    #[test]
    fn parses_write_hook_from_agents_md() {
        let reg = HookRegistry::from_agents_md(SAMPLE_AGENTS_MD);
        assert!(reg.len() >= 1);
        let msgs = reg.check("write", "02_distill/cards/article.md");
        assert!(!msgs.is_empty(), "should match card write");
        assert!(msgs[0].contains("NEXT:"));
    }

    #[test]
    fn parses_immutable_hook() {
        let reg = HookRegistry::from_agents_md(SAMPLE_AGENTS_MD);
        let msgs = reg.check("write", "01_capture/influential/article.md");
        assert!(!msgs.is_empty(), "should warn about immutable");
        assert!(msgs[0].contains("immutable"));
    }

    #[test]
    fn excludes_templates() {
        let reg = HookRegistry::from_agents_md(SAMPLE_AGENTS_MD);
        let msgs = reg.check("write", "02_distill/cards/_card-template.md");
        assert!(msgs.is_empty(), "template should be excluded");
    }

    #[test]
    fn no_match_for_unrelated() {
        let reg = HookRegistry::from_agents_md(SAMPLE_AGENTS_MD);
        let msgs = reg.check("write", "contacts/cont_001.json");
        assert!(msgs.is_empty());
    }

    #[test]
    fn no_match_for_read() {
        let reg = HookRegistry::from_agents_md(SAMPLE_AGENTS_MD);
        let msgs = reg.check("read", "02_distill/cards/article.md");
        assert!(msgs.is_empty(), "read should not trigger write hook");
    }

    #[test]
    fn fallback_when_no_explicit_rules() {
        let reg = HookRegistry::from_agents_md("This repo has cards and threads in distill folder.");
        let msgs = reg.check("write", "02_distill/cards/article.md");
        assert!(!msgs.is_empty(), "fallback should fire");
    }

    #[test]
    fn empty_on_irrelevant_agents_md() {
        let reg = HookRegistry::from_agents_md("Just a normal project. No special rules.");
        assert!(reg.is_empty());
    }
}
