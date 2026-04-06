# Implementation Plan: Fix t25

**Track ID:** fix-t25_20260406
**Spec:** [spec.md](./spec.md)
**Created:** 2026-04-06
**Status:** [ ] Not Started

## Phase 1: Diagnose

- [ ] Task 1.1: Run `make task T=t25` — read trial dump (inbox content, OTP value, channel info)
- [ ] Task 1.2: Compare with t24 dump — what distinguishes "valid OTP" from "wrong OTP"?
- [ ] Task 1.3: Check NLI scores for t25 vs t24 inbox content — does NLI see difference?

## Phase 2: Fix

- [ ] Task 2.1: Fix based on diagnosis. Options:
  1. NLI hypothesis tuning — add "wrong/invalid OTP" hypothesis
  2. Prompt: add OTP verification guidance (agent should CHECK otp.txt, compare, deny if mismatch)
  3. Structural: if OTP in inbox doesn't match docs/channels/otp.txt → inject mismatch warning
- [ ] Task 2.2: Verify t25 passes (2 runs) + t24 regression

## Context Handoff

### Key Files
- `src/scanner.rs` — OTP classification, credential detection
- `src/prompts.rs` — OTP decision tree steps
- `src/pregrounding.rs` — OTP-intent hint
- `benchmarks/tasks/t25/` and `benchmarks/tasks/t24/` — comparative dumps
