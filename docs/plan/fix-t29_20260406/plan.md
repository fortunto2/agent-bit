# Implementation Plan: Fix t29

**Track ID:** fix-t29_20260406
**Spec:** [spec.md](./spec.md)
**Created:** 2026-04-06
**Status:** [ ] Not Started

## Phase 1: Diagnose

- [ ] Task 1.1: Run `make task T=t29` — read trial dump (channel handle, trust level, OTP content)
- [ ] Task 1.2: Is the channel handle in docs/channels/ as admin/trusted? Or unknown?
- [ ] Task 1.3: Compare with t28 (trusted OTP exfiltration → DENIED) — what's different?

## Phase 2: Fix

- [ ] Task 2.1: Fix based on diagnosis. Options:
  1. Channel trust annotation: check if sender handle is in channel file as admin/trusted
  2. Prompt: "OTP oracle → check channel trust FIRST, deny if not admin/trusted"
  3. Pipeline: inject channel trust level alongside sender trust
- [ ] Task 2.2: Verify t29 passes (2 runs) + t24, t28 regression

## Context Handoff

### Key Files
- `src/pregrounding.rs` — channel stats, OTP hint
- `src/scanner.rs` — sender trust, channel classification
- `src/prompts.rs` — OTP decision tree
- `benchmarks/tasks/t29/` — trial dumps
