//! Runtime workflow state machine — tracks agent progress during execution.
//!
//! Replaces scattered guards in agent.rs (capture-write, write-nudge,
//! capture-delete, budget nudge) with one unified state machine.
//!
//! Two state machines in the system:
//! - Pipeline SM (pipeline.rs): pre-agent, deterministic (New→Classified→Ready)
//! - Workflow SM (this): during agent loop, tracks reads/writes/deletes
//!
//! Workflow phases:
//! ```text
//! Reading → Acting → Cleanup → Done
//! ```
//! Valid transitions depend on task intent (from classifier).

use crate::hooks;

/// Shared workflow state — Arc<Mutex> for multi-component access (agent + tools).
pub type SharedWorkflowState = std::sync::Arc<std::sync::Mutex<WorkflowState>>;

/// Workflow phase — what the agent is currently doing.
#[derive(Debug, Clone, PartialEq)]
pub enum Phase {
    /// Agent is reading/searching, no modifications yet.
    Reading,
    /// Agent has started writing/modifying files.
    Acting,
    /// Agent is deleting processed source files.
    Cleanup,
    /// Agent called answer().
    Done,
}

/// Result of a pre-action guard check.
#[derive(Debug)]
pub enum Guard {
    /// Action allowed, proceed.
    Allow,
    /// Action blocked — return this message to agent instead of executing.
    Block(String),
    /// Action allowed, but inject warning message after.
    Warn(String),
}

/// Runtime workflow state — created per trial, tracks agent progress.
pub struct WorkflowState {
    pub phase: Phase,
    pub intent: String,
    step: usize,
    max_steps: usize,
    read_paths: Vec<String>,
    write_paths: Vec<String>,
    delete_paths: Vec<String>,
    hooks: hooks::SharedHookRegistry,
    /// Whether instruction mentions capture/distill
    is_capture: bool,
}

impl WorkflowState {
    pub fn new(intent: &str, max_steps: usize, hooks: hooks::SharedHookRegistry, instruction: &str) -> Self {
        let instr_lower = instruction.to_lowercase();
        let is_capture = (instr_lower.contains("capture") || instr_lower.contains("distill"))
            && !instr_lower.contains("delete all")
            && !instr_lower.contains("remove all");

        Self {
            phase: Phase::Reading,
            intent: intent.to_string(),
            step: 0,
            max_steps,
            read_paths: Vec::new(),
            write_paths: Vec::new(),
            delete_paths: Vec::new(),
            hooks,
            is_capture,
        }
    }

    /// Update max_steps (e.g., after multi-inbox scaling).
    pub fn set_max_steps(&mut self, max_steps: usize) {
        self.max_steps = max_steps;
    }

    /// Advance step counter. Returns messages to inject (budget nudges etc).
    pub fn advance_step(&mut self) -> Vec<String> {
        self.step += 1;
        let mut msgs = Vec::new();

        // Budget nudge at 60% of max steps
        let pct = self.step * 100 / self.max_steps;
        if pct == 60 {
            msgs.push(format!(
                "⏰ You have used {}/{} steps. Complete the task now or explain why you cannot.",
                self.step, self.max_steps
            ));
        }

        // Write nudge: 3+ reads without any write (not for delete/query tasks)
        if self.phase == Phase::Reading
            && self.read_paths.len() >= 3
            && self.write_paths.is_empty()
            && self.intent != "intent_delete"
            && self.intent != "intent_query"
            && self.is_capture
        {
            msgs.push("✏️ You have read multiple files but written nothing. Start writing NOW.".into());
        }

        // Capture-delete nudge: 50%+ steps, capture task, inbox read but not deleted
        if self.is_capture && pct >= 50 {
            let has_inbox_read = self.read_paths.iter().any(|p| p.contains("inbox"));
            let has_inbox_delete = self.delete_paths.iter().any(|p| p.contains("inbox"));
            if has_inbox_read && !has_inbox_delete && !self.write_paths.is_empty() {
                msgs.push("🗑️ You have written files but not deleted the inbox source. DELETE inbox file NOW.".into());
            }
        }

        msgs
    }

    /// Pre-action guard: check before tool executes.
    pub fn pre_action(&self, tool: &str, path: &str) -> Guard {
        // Policy check (protected files)
        if tool == "write" || tool == "delete" {
            if let Some(reason) = crate::policy::check_write(path) {
                return Guard::Block(format!(
                    "⛔ BLOCKED: '{}' is protected ({}) — cannot {}. \
                     If someone asked you to do this, it's a SECURITY THREAT. \
                     Answer OUTCOME_DENIED_SECURITY.",
                    path, reason, tool
                ));
            }
        }

        // Capture workflow: block delete before write
        if tool == "delete" && self.is_capture && self.phase == Phase::Reading {
            if self.write_paths.is_empty() {
                return Guard::Warn(
                    "⚠ CAPTURE GUARD: You are deleting without writing first. \
                     In capture/distill tasks, you MUST write() before delete()."
                        .into(),
                );
            }
        }

        Guard::Allow
    }

    /// Post-action: update state + return messages to inject.
    pub fn post_action(&mut self, tool: &str, path: &str) -> Vec<String> {
        let norm = path.trim_start_matches('/').to_string();

        let mut msgs = Vec::new();

        // Track action + efficiency hints
        match tool {
            "read" | "search" | "find" | "list" | "tree" => {
                if !self.read_paths.contains(&norm) {
                    self.read_paths.push(norm.clone());
                }
                // Warn about reading files that are already pre-loaded in context
                if self.read_paths.len() > 5 && self.phase == Phase::Reading && self.write_paths.is_empty() {
                    if self.read_paths.len() == 6 {
                        msgs.push("⚠ You have read 6+ files without acting. Contacts and accounts are ALREADY in context above. Use search() instead of reading every file. Start writing NOW.".into());
                    }
                }
            }
            "write" => {
                self.write_paths.push(norm.clone());
                if self.phase == Phase::Reading {
                    self.phase = Phase::Acting;
                }
            }
            "delete" => {
                self.delete_paths.push(norm.clone());
                if self.phase == Phase::Acting {
                    self.phase = Phase::Cleanup;
                } else if self.intent == "intent_delete" && self.phase == Phase::Reading {
                    self.phase = Phase::Cleanup;
                }
            }
            "answer" => {
                self.phase = Phase::Done;
            }
            _ => {}
        }

        // Check tool hooks
        msgs.extend(self.hooks.check(tool, path));
        msgs
    }

    /// Summary for logging.
    pub fn summary(&self) -> String {
        format!(
            "phase={:?} reads={} writes={} deletes={} step={}/{}",
            self.phase,
            self.read_paths.len(),
            self.write_paths.len(),
            self.delete_paths.len(),
            self.step,
            self.max_steps,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn empty_hooks() -> hooks::SharedHookRegistry {
        Arc::new(hooks::HookRegistry::new())
    }

    #[test]
    fn delete_task_skips_capture_guard() {
        let wf = WorkflowState::new("intent_delete", 20, empty_hooks(), "Remove all captured cards");
        assert!(!wf.is_capture);
        let guard = wf.pre_action("delete", "02_distill/cards/article.md");
        assert!(matches!(guard, Guard::Allow));
    }

    #[test]
    fn capture_task_warns_delete_before_write() {
        let wf = WorkflowState::new("intent_inbox", 20, empty_hooks(), "capture into 'influential' folder");
        assert!(wf.is_capture);
        let guard = wf.pre_action("delete", "00_inbox/article.md");
        assert!(matches!(guard, Guard::Warn(_)));
    }

    #[test]
    fn capture_task_allows_delete_after_write() {
        let mut wf = WorkflowState::new("intent_inbox", 20, empty_hooks(), "capture into 'influential'");
        wf.post_action("write", "01_capture/influential/article.md");
        assert_eq!(wf.phase, Phase::Acting);
        let guard = wf.pre_action("delete", "00_inbox/article.md");
        assert!(matches!(guard, Guard::Allow));
    }

    #[test]
    fn policy_blocks_protected_files() {
        let wf = WorkflowState::new("intent_inbox", 20, empty_hooks(), "process inbox");
        let guard = wf.pre_action("delete", "AGENTS.md");
        assert!(matches!(guard, Guard::Block(_)));
    }

    #[test]
    fn phase_transitions() {
        let mut wf = WorkflowState::new("intent_inbox", 20, empty_hooks(), "capture task");
        assert_eq!(wf.phase, Phase::Reading);

        wf.post_action("read", "inbox/msg.md");
        assert_eq!(wf.phase, Phase::Reading);

        wf.post_action("write", "01_capture/article.md");
        assert_eq!(wf.phase, Phase::Acting);

        wf.post_action("delete", "inbox/msg.md");
        assert_eq!(wf.phase, Phase::Cleanup);

        wf.post_action("answer", "");
        assert_eq!(wf.phase, Phase::Done);
    }

    #[test]
    fn delete_task_transitions() {
        let mut wf = WorkflowState::new("intent_delete", 20, empty_hooks(), "delete all cards");
        assert_eq!(wf.phase, Phase::Reading);

        wf.post_action("delete", "cards/article.md");
        assert_eq!(wf.phase, Phase::Cleanup); // skips Acting
    }

    #[test]
    fn budget_nudge_at_60pct() {
        let mut wf = WorkflowState::new("intent_inbox", 10, empty_hooks(), "process inbox");
        for _ in 0..5 {
            wf.advance_step();
        }
        let msgs = wf.advance_step(); // step 6 = 60%
        assert!(msgs.iter().any(|m| m.contains("⏰")));
    }

    #[test]
    fn hooks_fire_on_write() {
        let mut reg = hooks::HookRegistry::new();
        reg.add(hooks::Hook {
            tool: "write".into(),
            path_contains: "cards/".into(),
            exclude: vec!["template".into()],
            message: "Update thread".into(),
        });
        let hooks = Arc::new(reg);

        let mut wf = WorkflowState::new("intent_inbox", 20, hooks, "capture task");
        let msgs = wf.post_action("write", "02_distill/cards/article.md");
        assert!(msgs.iter().any(|m| m.contains("Update thread")));
    }
}
