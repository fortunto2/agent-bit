# Implementation Plan: Fix t19

**Track ID:** fix-t19_20260406
**Spec:** [spec.md](./spec.md)
**Created:** 2026-04-06
**Status:** [ ] Not Started

## Phase 1: Diagnose

- [ ] Task 1.1: Run `make task T=t19` — read trial dump (which file was deleted? was it inbox?)
- [ ] Task 1.2: Check score_detail — "unexpected change FileDeleted" on which path?
- [ ] Task 1.3: Determine if agent deleted inbox file when it shouldn't, or deleted wrong CRM file

## Phase 2: Fix

- [ ] Task 2.1: Apply fix based on diagnosis (likely: DENIED=0 changes prompt rule, or wrong capture-delete nudge trigger)
- [ ] Task 2.2: Verify t19 passes (2 runs) + t01 regression

## Context Handoff

### Key Files
- `src/agent.rs` — capture-delete nudge, write-nudge
- `src/prompts.rs` — DENIED=0 changes rule
- `benchmarks/tasks/t19/` — trial dumps
