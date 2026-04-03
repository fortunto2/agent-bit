# Specification: Fix t03 Non-Deterministic File Ops Failure

**Track ID:** fix-t03-file-ops_20260403
**Type:** Bug
**Created:** 2026-04-03
**Status:** Draft

## Summary

t03 ("capture from inbox, distill, delete") fails non-deterministically on Nemotron because the LLM misclassifies the task_type in the Router reasoning step. When classified as "search", write/delete tools are permanently unavailable and the model cannot complete file modifications. Additionally, the system prompt lacks an explicit capture/distill/delete workflow example, leaving Nemotron without a concrete pattern to follow for multi-file operations.

The fix addresses three layers: (1) Router tool-gating safety net — prevent permanent write/delete lockout on any task type, (2) task_type classification hints — make "capture/distill/process" map to "edit", (3) prompt examples — add a concrete capture-distill-delete workflow.

## Acceptance Criteria

- [ ] AC1: "search" task_type with step > 0 has write/delete tools available (safety net against misclassification)
- [ ] AC2: Reasoning tool task_type description includes "capture", "distill", "process inbox" as "edit" cues
- [ ] AC3: Default CRM examples include a capture-distill-delete workflow showing read → write → delete → answer
- [ ] AC4: PLANNING_PROMPT includes a "capture from inbox" common pattern
- [ ] AC5: `cargo test` passes (all 105+ tests)
- [ ] AC6: `make task T=t03` passes on Nemotron (at least 2/3 runs)

## Dependencies

- None (prompt + Router changes only, no new crates)

## Out of Scope

- t08 "delete that card" ambiguity (separate track)
- t25/t29 OTP edge cases (separate tracks)
- Blocking OutcomeValidator (roadmap architecture item)
- Reflexion changes (standard mode only, Nemotron uses explicit)

## Technical Notes

- **Router tool gating** (`src/agent.rs:321-364`): "search" permanently restricts to read-only; "analyze" step 0 is read-only then opens up; "edit" has write/delete from step 0. Fix: make "search" behave like "analyze" — read-only on step 0, full toolkit on step 1+.
- **examples_for_class** (`src/main.rs:112-153`): Returns dynamic examples based on ML classifier label. For t03, instruction is "crm" class → gets default examples (CRM lookup, Email writing, Counting). No capture/distill example exists.
- **PLANNING_PROMPT** (`src/main.rs:1214-1232`): Lists common patterns but missing "capture from inbox, distill, write, delete" pattern.
- **Reasoning tool** (`src/agent.rs:90-135`): task_type description at line 110 says `search=find/read, edit=modify files, analyze=multi-step`. Needs "capture/distill/delete/process" added as "edit" cues.
- **Roadmap rule**: "prefer prompt wording > classifier tuning > structural signals > new code". This fix follows that order.
- **No task-ID checks**: All changes must be universal patterns, not t03-specific.
