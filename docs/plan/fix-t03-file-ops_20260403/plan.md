# Implementation Plan: Fix t03 Non-Deterministic File Ops Failure

**Track ID:** fix-t03-file-ops_20260403
**Spec:** [spec.md](./spec.md)
**Created:** 2026-04-03
**Status:** [~] In Progress — Phase 1-3 complete, Phase 4 added by review

## Overview

Three-layer fix following "prompt wording > structural signals > new code" principle: (1) make Router tool gating less aggressive for "search" task_type, (2) improve task_type classification hints, (3) add capture/distill workflow examples.

## Phase 1: Router Safety Net

Prevent permanent write/delete lockout when Nemotron misclassifies task_type as "search". Make "search" behave like "analyze" — read-only on step 0, full toolkit after.

### Tasks

- [x] Task 1.1: In `src/agent.rs`, change the "search" arm of the Router match to gate write/delete behind `step > 0`. Extracted `filter_tools_for_task()` for testability. <!-- sha:385aed2 -->

- [x] Task 1.2: Add unit tests for Router tool filtering — 6 tests verify search/edit/analyze/security tool availability per step. <!-- sha:385aed2 -->

### Verification
- [x] `cargo test` passes (111 tests)
- [x] Router tests verify tool availability per task_type + step

## Phase 2: Prompt & Classification Hints

Improve task_type classification and add capture/distill workflow examples.

### Tasks

- [x] Task 2.1: Updated reasoning tool's `task_type` description with capture/distill/process inbox cues. <!-- sha:a1cf2ef -->

- [x] Task 2.2: Added capture-from-inbox workflow example to default CRM examples. <!-- sha:a1cf2ef -->

- [x] Task 2.3: Added capture/distill pattern to PLANNING_PROMPT common patterns. <!-- sha:a1cf2ef -->

### Verification
- [x] `cargo test` passes (111 tests)
- [x] Prompt strings updated correctly (grep for "capture" in main.rs)

## Phase 3: Integration Testing & Docs

### Tasks

- [x] Task 3.1: Run `make task T=t03` on Nemotron — 0/3 pass. Root cause: agent loops on thread file reads (6x) without writing. Router tool gating works (writes at steps 4,7), deeper prompt issue remains. <!-- sha:n/a (test-only) -->

- [x] Task 3.2: Run regression on t01 (`make task T=t01`) — passes (1.00, 5 steps). No regression. <!-- sha:n/a (test-only) -->

- [x] Task 3.3: Update CLAUDE.md — added capture/distill workflow section, kept t03 in failing tasks with root cause, updated test count to 112. <!-- sha:0916444 -->

### Verification
- [ ] t03 passes 2/3+ runs on Nemotron — **FAILED**: 0/3, deeper issue (thread update loop)
- [x] t01 regression passes (1.00, 5 steps)
- [x] CLAUDE.md updated <!-- sha:0916444 -->

## Final Verification
- [ ] All acceptance criteria from spec met — **PARTIAL**: Router safety net + prompt hints work, but t03 still fails (thread update loop)
- [x] Tests pass (cargo test) — 112 pass
- [x] Build succeeds (cargo build)
- [ ] t03 deterministic on Nemotron — **FAILED**: 0/3 pass, needs new plan for thread update workflow

## Phase 4: Thread-Update Loop Fix (added by review)

Root cause: Nemotron reads AGENT_EDITABLE sections of thread files 6x without writing. The model sees existing content but doesn't realize it needs to update/append. Needs either a prompt example for thread updates or a write-after-read nudge.

### Tasks

- [ ] Task 4.1: Analyze t03 agent logs to identify exact loop pattern — which file is re-read, what AGENT_EDITABLE content triggers the loop
- [ ] Task 4.2: Add thread-update prompt example to default CRM examples: read(thread) → write(thread with new entry) pattern
- [ ] Task 4.3: Consider adaptive nudge after 3+ consecutive reads of same file without intervening write — inject "You've read this file N times. If you need to update it, use write() now."
- [ ] Task 4.4: Run `make task T=t03` on Nemotron — target 2/3 pass

### Verification
- [ ] t03 passes 2/3+ runs on Nemotron (AC6)
- [ ] No regression on t01

## Context Handoff

### Session Intent
Fix t03 non-deterministic failure where Nemotron can't complete file operations because Router permanently locks out write/delete tools when task_type is misclassified as "search".

### Key Files
- `src/agent.rs` — Router tool filtering (lines 321-364), reasoning_tool_def task_type description (line 110)
- `src/main.rs` — examples_for_class (lines 112-153), PLANNING_PROMPT (lines 1214-1232)
- `CLAUDE.md` — failing tasks section, key design decisions

### Decisions Made
- **Safety net over prompt-only**: Prompt improvements help Nemotron classify correctly, but the Router safety net (unlocking write/delete after step 0 for "search") prevents catastrophic failure if misclassification still happens. Defense-in-depth.
- **"search" mirrors "analyze" pattern**: Step 0 read-only, step 1+ full toolkit. This is the minimal change — no new task_type values, no new routing logic.
- **Universal examples**: The capture/distill example uses generic CRM patterns (search → read → write → delete), not t03-specific data.
- **No classifier changes**: The ML classifier labels ("crm", "injection", etc.) are unchanged. Only the reasoning tool's task_type guidance is improved.

### Risks
- Relaxing "search" tool gating could theoretically let the model make unwanted writes on pure search tasks. Mitigated: step 0 remains read-only, so the model's first action is still read-only. By step 1, the model has seen the data and can make informed decisions.
- Adding more examples increases prompt length. Mitigated: one example adds ~200 chars, well within budget.

---
_Generated by /plan. Tasks marked [~] in progress and [x] complete by /build._
