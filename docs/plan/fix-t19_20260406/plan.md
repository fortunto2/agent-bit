# Implementation Plan: Fix t19

**Track ID:** fix-t19_20260406
**Spec:** [spec.md](./spec.md)
**Created:** 2026-04-06
**Status:** [ ] Not Started

## Phase 1: Diagnose

- [x] Task 1.1: Run `make task T=t19` — read trial dump (which file was deleted? was it inbox?) <!-- sha:diag -->
- [x] Task 1.2: Check score_detail — "unexpected change FileDeleted" on inbox/msg_001.txt <!-- sha:diag -->
- [x] Task 1.3: Root cause: 3 issues — (a) pre-grounding "MUST delete" for all inbox, (b) capture-delete nudge triggers on "inbox" keyword, (c) reflexion flips safe→blocked <!-- sha:diag -->

## Phase 2: Fix

- [~] Task 2.1a: Fix pre-grounding — conditional delete reminder only for capture/distill/delete instructions
- [ ] Task 2.1b: Fix capture-delete nudge — remove "inbox" trigger, keep capture/distill only
- [ ] Task 2.1c: Fix reflexion — change default prompt_mode to "explicit"
- [ ] Task 2.2: Verify t19 passes (2 runs) + t03 regression

## Context Handoff

### Key Files
- `src/agent.rs` — capture-delete nudge, write-nudge
- `src/prompts.rs` — DENIED=0 changes rule
- `benchmarks/tasks/t19/` — trial dumps
