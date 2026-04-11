//! PAC1-specific hook extensions on top of sgr-agent::hooks.
//!
//! Adds AGENTS.MD parser to populate sgr-agent's generic HookRegistry.
//! Re-exports sgr-agent types for convenience.

pub use sgr_agent::hooks::{Hook, HookRegistry, Shared as SharedHookRegistry};

/// Parse AGENTS.MD content to extract workflow hooks.
///
/// Recognizes patterns:
/// - "When adding/writing to {path}, also {action}" → write completion hook
/// - "Keep files in {path} immutable" → write warning hook
pub fn from_agents_md(content: &str) -> HookRegistry {
    let mut registry = HookRegistry::new();

    for line in content.lines() {
        let ll = line.to_lowercase();

        // Pattern: "when adding/writing to {path}, also {action}"
        if (ll.contains("when adding") || ll.contains("when writing") || ll.contains("after writing"))
            && ll.contains("also")
        {
            if let Some(source_path) = extract_path_ref(line) {
                if let Some(action_part) = line.to_lowercase().split("also").nth(1) {
                    let next_step = action_part.trim().trim_start_matches(',').trim();
                    registry.add(Hook {
                        tool: "write".into(),
                        path_contains: source_path.trim_matches('/').to_lowercase(),
                        exclude: vec!["template".into()],
                        message: format!("NEXT: {} (from AGENTS.MD)", next_step),
                    });
                    if let Some(target) = extract_path_ref(&action_part) {
                        eprintln!("  📎 Hook: write to {} → {}", source_path, target);
                    }
                }
            }
        }

        // Pattern: "keep files in {path} immutable"
        if ll.contains("immutable") || ll.contains("do not modify") {
            if let Some(path) = extract_path_ref(line) {
                registry.add(Hook {
                    tool: "write".into(),
                    path_contains: path.trim_matches('/').to_lowercase(),
                    exclude: vec![],
                    message: format!("⚠ WARNING: files in {} are immutable per AGENTS.MD.", path),
                });
                eprintln!("  📎 Hook: {} → immutable warning", path);
            }
        }
    }

    // Built-in: outbox seq.json update after email write
    // AI-NOTE: universal rule — any workspace with outbox/ needs seq.json bumped after email write.
    // Activated when AGENTS.MD mentions outbox/seq/email. Hook → tool output.
    {
        let full_lower = content.to_lowercase();
        if full_lower.contains("outbox") || full_lower.contains("seq.json") || full_lower.contains("email") {
            registry.add(Hook {
                tool: "write".into(),
                path_contains: "outbox/".into(),
                exclude: vec!["seq.json".into(), "readme".into()],
                message: "NEXT: update outbox/seq.json (increment ID by 1).".into(),
            });
            eprintln!("  📎 Hook (built-in): outbox write → seq.json reminder");
        }
    }

    // Fallback: cards + threads pattern
    if registry.is_empty() {
        let lower = content.to_lowercase();
        if lower.contains("card") && lower.contains("thread") && lower.contains("distill") {
            registry.add(Hook {
                tool: "write".into(),
                path_contains: "distill/cards/".into(),
                exclude: vec!["template".into()],
                message: "NEXT: update matching thread in 02_distill/threads/.".into(),
            });
            eprintln!("  📎 Hook (fallback): distill/cards → threads");
        }
    }

    registry
}

/// Extract first path-like reference from text.
fn extract_path_ref(text: &str) -> Option<String> {
    // Markdown link: [text](/path/)
    if let Some(start) = text.find("](/") {
        if let Some(end) = text[start + 2..].find(')') {
            return Some(text[start + 2..start + 2 + end].to_string());
        }
    }
    // Plain path
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

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
Keep existing files in `01_capture/` immutable.
When adding a card under [/02_distill/cards/](/02_distill/cards/), also update threads under [/02_distill/threads/](/02_distill/threads/).
";

    #[test]
    fn parses_write_hook() {
        let reg = from_agents_md(SAMPLE);
        let msgs = reg.check("write", "02_distill/cards/article.md");
        assert!(!msgs.is_empty());
    }

    #[test]
    fn parses_immutable() {
        let reg = from_agents_md(SAMPLE);
        let msgs = reg.check("write", "01_capture/influential/article.md");
        assert!(!msgs.is_empty());
        assert!(msgs[0].contains("immutable"));
    }

    #[test]
    fn excludes_templates() {
        let reg = from_agents_md(SAMPLE);
        assert!(reg.check("write", "02_distill/cards/_card-template.md").is_empty());
    }

    #[test]
    fn fallback() {
        let reg = from_agents_md("This has cards and threads in distill.");
        assert!(!reg.check("write", "distill/cards/x.md").is_empty());
    }
}
