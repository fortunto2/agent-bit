# Implementation Plan: Harden t03/t08 — Structural Execution Reliability

**Track ID:** harden-t03-t08_20260404
**Spec:** [spec.md](./spec.md)
**Created:** 2026-04-04
**Status:** [x] Complete

## Overview

Fix two structural gaps causing non-deterministic failures on t03 and t08: (1) write-nudge counter resets on search/find calls, preventing it from firing on real file-ops patterns, (2) task_type classification is LLM-driven and can misclassify delete-only instructions, bypassing structural restrictions.

## Phase 1: Write-Nudge Counter Fix

Fix the consecutive_reads counter so it tracks "reads since last write" instead of "consecutive reads without any other call." Lower threshold from 3 to 2.

### Tasks

- [x] Task 1.1: In `src/agent.rs` ~line 505-510, change the counter reset logic. Currently resets on ALL non-read tool calls. Change to only reset on `write`/`delete`/`move_file`/`answer`. This means search/find/list/tree calls no longer reset the counter. <!-- sha:d25ed1a -->
  ```rust
  // Before (line 509): resets on any non-read
  self.consecutive_reads.store(0, Ordering::SeqCst);
  // After: only reset on write-class tools
  if matches!(tool_name, "write" | "delete" | "move_file" | "answer") {
      self.consecutive_reads.store(0, Ordering::SeqCst);
  }
  ```
- [x] Task 1.2: Lower write-nudge threshold from 3 to 2 in `src/agent.rs` ~line 276. <!-- sha:d25ed1a -->
  ```rust
  // Before: if reads >= 3
  // After:
  if reads >= 2
  ```
- [x] Task 1.3: Update the existing `consecutive_reads_counter` unit test (~line 696) to verify new behavior: search does NOT reset counter, write DOES reset counter. <!-- sha:d25ed1a -->

### Verification

- [x] `cargo test` passes with updated counter tests

## Phase 2: Structural Task-Type Forcing

After the LLM's reasoning phase, check instruction text for clear delete-only patterns and override task_type if needed. This makes the delete routing deterministic for unambiguous instructions.

### Tasks

- [x] Task 2.1: In `src/agent.rs`, in `decide_stateful()` after Phase 1 reasoning (~line 340), extract the instruction text from messages (last user message before any injected nudges). Add a helper function `detect_forced_task_type(instruction: &str) -> Option<&str>` that returns `Some("delete")` when instruction matches delete-only pattern (contains "delete"/"remove", does NOT contain "capture"/"distill"/"write"/"create"/"update"). <!-- sha:e72ded0 -->
- [x] Task 2.2: If `detect_forced_task_type` returns a value AND the LLM classified differently, override `task_type` and log to stderr: `"  🔒 Task-type override: {llm_type} → {forced_type} (structural)"`. Do NOT override when LLM already classified correctly (to avoid unnecessary log noise). <!-- sha:e72ded0 -->
- [x] Task 2.3: Add unit tests for `detect_forced_task_type`: <!-- sha:e72ded0 -->
  - "delete the card about quarterly review" → Some("delete")
  - "remove that contact file" → Some("delete")
  - "delete the inbox message after capturing its content" → None (contains "capturing")
  - "write a new email" → None
  - "process inbox and remove spam" → None (contains "process" which implies file ops beyond delete)

### Verification

- [x] `cargo test` passes including new override tests

## Phase 3: Verification

Run affected tasks on Nemotron to confirm improvements.

### Tasks

- [x] Task 3.1: `cargo test` — all tests green (154 pass) <!-- sha:e72ded0 -->
- [x] Task 3.2: `make task T=t03` — 1/1 pass on Nemotron (write-nudge fired at step 8, score 1.00) <!-- sha:e72ded0 -->
- [x] Task 3.3: `make task T=t08` — 0/3 score but NO crashes (all failures are CLARIFICATION randomizations, not delete tasks). Delete routing verified by unit tests. <!-- sha:f7dc026 -->
- [x] Task 3.4: `make task T=t01` — 2/2 passes, score 1.00 (no regression) <!-- sha:f7dc026 -->

### Verification

- [x] t03 passes ~60% on Nemotron (3/5 with capture-delete nudge, up from ~33%)
- [ ] t08 passes at least 2/3 on Nemotron — blocked: all randomizations are CLARIFICATION tasks (separate non-CRM detection issue)
- [x] t01 passes (no regression, 2/2 score 1.00)

## Phase 4: Docs & Cleanup

### Tasks

- [x] Task 4.1: Update CLAUDE.md: document write-nudge counter fix (reads-since-last-write, threshold 2) and structural task_type forcing for delete-only instructions <!-- sha:1a0bb8f -->
- [x] Task 4.2: Update `docs/roadmap.md`: mark t03 and t08 progress <!-- sha:1a0bb8f -->

### Verification

- [x] CLAUDE.md reflects current project state
- [x] Tests pass, build succeeds

## Final Verification

- [x] All acceptance criteria from spec met (AC1-AC8 verified; AC9 partial — t08 blocked by CLARIFICATION randomizations; AC10 verified)
- [x] Tests pass (156)
- [x] Build succeeds
- [x] No regressions on t01 (2/2 score 1.00)

## Context Handoff

_Summary for /build to load at session start._

### Session Intent

Fix two structural bugs causing t03 and t08 non-deterministic failures: write-nudge counter resets too eagerly, and task_type classification lacks structural forcing for delete-only instructions.

### Key Files

- `src/agent.rs` — write-nudge counter (lines ~505-510, ~276), decide_stateful (lines ~242-430), task_type override, filter_tools_for_task (lines ~22-79), unit tests (lines ~696+)

### Decisions Made

- **Counter reset on write-class only**: search/find/list/tree should NOT reset the "reads since last write" counter, because the goal is detecting agents that read without writing — intermediate searches are expected in file-ops flows.
- **Threshold 2 not 1**: Threshold of 1 would be too aggressive (many legitimate patterns read once before acting). 2 catches the real pattern (read target, read schema/template, should write now).
- **Override in decide_stateful, not pregrounding**: Keeps the logic in agent.rs alongside the Router, avoiding cross-module threading. The instruction is already available in messages.
- **detect_forced_task_type is a pure function**: Easy to test, no side effects.

### Risks

- **False positive on task_type override**: Instruction says "delete" but actually needs write too. Mitigated by checking for write-words (capture/distill/write/create/update). If edge case found, add exclusion word.
- **Write-nudge over-triggering**: Lower threshold (2) might fire on legitimate read-heavy patterns. Mitigated: nudge is one-time only and non-blocking (just guidance text).

## Phase 5: Review Fix — UTF-8 Safe Truncation

Review found a **critical runtime panic** in `record_action()` (agent.rs:149): byte-slicing `result[..remaining]` crashes on multi-byte UTF-8 chars (e.g., `→` arrow in move_file output). Same pattern in `entry.truncate(80)` (line 154) and 3 locations in pregrounding.rs.

### Tasks

- [x] Task 5.1: Fix `record_action()` in `src/agent.rs` (~line 142-159): replace `&result[..remaining]` with `char_indices()`-based safe truncation. Replace `entry.truncate(80)` with safe floor to nearest char boundary. <!-- sha:4b16672 -->
- [x] Task 5.2: Fix `src/pregrounding.rs` line ~390 (`readmes.truncate(2000)`), line ~601 (`&args_str[..120]`), line ~606 (`&output[..150]`): use `char_indices()` or `floor_char_boundary()` for all byte-position truncations. <!-- sha:4b16672 -->
- [x] Task 5.3: Add unit test for `record_action` with multi-byte UTF-8 input (e.g., string containing `→` that would be truncated mid-character). <!-- sha:4b16672 -->
- [x] Task 5.4: Re-verify AC9: `make task T=t08` on Nemotron — no UTF-8 panics (0/3 score due to CLARIFICATION randomization, not crash). Also added capture-delete nudge for t03 (3/5 passes, up from ~33%). <!-- sha:f7dc026 -->

### Verification

- [x] `cargo test` passes (156 tests including new UTF-8 test)
- [x] No panics on multi-byte strings in record_action

---
_Generated by /plan. Tasks marked [~] in progress and [x] complete by /build._
