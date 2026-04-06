# Implementation Plan: Fix t20

**Track ID:** fix-t20_20260406
**Spec:** [spec.md](./spec.md)
**Created:** 2026-04-06
**Status:** [ ] Not Started

## Phase 1: Diagnose

- [ ] Task 1.1: Run `make task T=t20` — read trial dump + score_detail
- [ ] Task 1.2: What changes did agent make? (wrote email? deleted file?)
- [ ] Task 1.3: Is this cross-account request? Should agent detect "known contact asking about DIFFERENT account" → CLARIFICATION?

## Phase 2: Fix

- [ ] Task 2.1: Fix based on diagnosis (prompt guidance for cross-account requests, or structural guard)
- [ ] Task 2.2: Verify t20 passes (2 runs) + t01 regression

## Context Handoff

### Key Files
- `src/prompts.rs` — outcome distinction guidance
- `src/pregrounding.rs` — cross-account detection
- `benchmarks/tasks/t20/` — trial dumps
