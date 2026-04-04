# Specification: Harden t03/t08 — Structural Execution Reliability

**Track ID:** harden-t03-t08_20260404
**Type:** Bug
**Created:** 2026-04-04
**Status:** Draft

## Summary

t03 ("capture from inbox, distill, delete") and t08 ("delete that card") remain non-deterministic on Nemotron despite 3 prior fix plans. Root-cause analysis reveals two structural gaps:

1. **Write-nudge counter bug** (t03): The `consecutive_reads` counter in `agent.rs:507-509` resets on ANY non-read tool call (search, find, list), not just write/delete. For t03's typical flow (read inbox → search contacts → read contact → should write), `search()` resets the counter to 0, so the write-nudge (threshold=3) never fires. The counter should track "reads since last write," not "consecutive reads without any other call."

2. **LLM-dependent task_type classification** (t08): Delete routing (`filter_tools_for_task("delete", ...)`) structurally removes write tools, preventing the "write instead of delete" failure mode. But the task_type comes from the LLM's reasoning tool — if Nemotron classifies as "edit" instead of "delete," the restriction is bypassed. Structural forcing would make this deterministic.

Both fixes follow the escalation discipline (suggestive → directive → structural) recommended by the retro. Prior plans used suggestive (examples) and directive (pre-grounding hints). This plan applies structural fixes.

## Acceptance Criteria

- [x] AC1: `consecutive_reads` counter only resets on write/delete/move/answer tool calls, not search/find/list/tree
- [x] AC2: Write-nudge threshold lowered from 3 to 2 (reads-since-last-write)
- [x] AC3: Task_type structurally forced to "delete" when instruction matches delete-only pattern (contains delete/remove, does NOT contain capture/distill/write/create)
- [x] AC4: Structural override logged to stderr for observability
- [x] AC5: Unit tests for counter reset logic (write resets, search does not reset)
- [x] AC6: Unit tests for task_type override (delete-only patterns)
- [x] AC7: `cargo test` passes (147+ tests)
- [x] AC8: `make task T=t03` passes on Nemotron (at least 2/3 runs)
- [ ] AC9: `make task T=t08` passes on Nemotron (at least 2/3 runs) — BLOCKED by UTF-8 panic in record_action
- [x] AC10: `make task T=t01` passes (no regression)

## Dependencies

- None (agent.rs changes only, no new crates)

## Out of Scope

- t25/t29 OTP handling (separate roadmap priority 3)
- OutcomeValidator seed expansion
- NLI model integration
- Pipeline/factory improvements (solo-lib.sh, solo-dev.sh)

## Technical Notes

- **Write-nudge counter** (`src/agent.rs:507-509`): Currently `fetch_add(1)` for read, `store(0)` for everything else. Fix: only reset on `write`/`delete`/`move_file`/`answer` tool names.
- **Structural task_type override** (`src/agent.rs:242-340`): After Phase 1 reasoning, check instruction text from messages for delete-only pattern. If LLM classified as non-"delete", override. The instruction is the last user message in the conversation.
- **No sgr-agent changes**: All modifications are in agent-bit's `src/agent.rs`.
- **Roadmap rule**: "prefer prompt wording > classifier tuning > structural signals > new code" — this is structural signals (3rd preference), applied after prompt/directive exhaustion.
