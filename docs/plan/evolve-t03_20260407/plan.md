# Implementation Plan: Stabilize t03

**Track ID:** evolve-t03_20260407
**Spec:** [spec.md](./spec.md)
**Created:** 2026-04-07
**Status:** [ ] Not Started

## Phase 1: Diagnose

- [ ] Task 1.1: Run `make task T=t03` 3 times, collect dumps + BitGN logs
- [ ] Task 1.2: Compare passing vs failing runs — what differs?
- [ ] Task 1.3: Check: write-nudge timing, capture-delete nudge, step budget

## Phase 2: Fix via /evolve

- [ ] Task 2.1: Run `/evolve t03` — iterate until 4/5 passes
- [ ] Task 2.2: Regression check t01, t09

## Context Handoff

### Key Files
- `src/agent.rs` — write-nudge counter, capture-delete nudge
- `src/pregrounding.rs` — intent hints, inbox processing guidance
- `benchmarks/tasks/t03/` — trial dumps
