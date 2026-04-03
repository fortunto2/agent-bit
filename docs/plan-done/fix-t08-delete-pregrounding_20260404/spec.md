# Specification: Fix t08 — Delete Routing + Pre-grounding (Iteration 2)

**Track ID:** fix-t08-delete-pregrounding_20260404
**Type:** Bug
**Created:** 2026-04-04
**Status:** Draft

## Summary

t08 ("delete that card") still fails 0/1 on Nemotron after the first prompt-only fix (fix-t08-delete-ambiguity_20260403). The observed failure mode: **agent captures/writes files instead of deleting**. Nemotron interprets "delete that card" as a capture/process task, using write() when only delete() is needed.

Prompt guidance alone wasn't enough (iteration 1 proved this). Per roadmap rule "prompt > classifier > structural > new code", this iteration escalates to **structural signals**: a new "delete" task_type in the Router that physically removes write/create tools, preventing the capture-instead-of-delete failure mode. Combined with a lightweight delete-intent pre-grounding hint.

## Acceptance Criteria

- [x] AC1: "delete" task_type exists in Router reasoning tool enum
- [x] AC2: "delete" routing restricts tools to search+read+find+list+delete+answer (NO write/mkdir/move)
- [x] AC3: task_type description clearly distinguishes "delete" (remove only) from "edit" (which includes capture/create/delete-plus-write)
- [x] AC4: Delete example in prompts includes explicit anti-pattern ("DO NOT write/create files when deleting")
- [x] AC5: Delete intent pre-grounding injects disambiguation reminder before LLM loop
- [x] AC6: `cargo test` passes (123 tests, including 3 new Router tests for "delete")
- [ ] AC7: `make task T=t08` passes at least 2/3 on Nemotron (task randomized this run — structural fix correct)
- [x] AC8: No regression on t03 (1.00 — capture-then-delete works via "edit" routing)

## Dependencies

- fix-t08-delete-ambiguity_20260403 (iteration 1, completed — prompt guidance already in place)
- fix-t03-file-ops_20260403 (completed — must not regress capture/distill/delete workflow)

## Out of Scope

- Delete confirmation tool (over-engineering — structural routing is sufficient)
- Full workspace search for target resolution (too complex — simple pre-grounding hint is enough)
- Blocking OutcomeValidator (separate architecture track)
- Fixing t03/t25/t29 (separate tasks, different root causes)

## Technical Notes

- `filter_tools_for_task()` in agent.rs:23 handles Router tool gating — add "delete" case
- `reasoning_tool_def()` in agent.rs:146 defines task_type enum — add "delete" variant
- task_type description at agent.rs:166 currently maps "delete" to "edit" — must carve out "delete" as separate
- Risk: if Nemotron misclassifies a capture+delete task (t03) as "delete" instead of "edit", it loses write tools → regression. Mitigation: description explicitly says "capture, distill = edit" and "delete = ONLY removing a file"
- Pre-grounding hint goes in pregrounding.rs:run_agent(), after instruction classification
- The "delete" task_type has NO step-based safety net (unlike "search" which opens up at step 1+) — this is intentional. The structural restriction IS the fix.
