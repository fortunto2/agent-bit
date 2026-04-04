# Implementation Plan: Harden OTP Handling (t25/t29)

**Track ID:** harden-otp-t25-t29_20260404
**Spec:** [spec.md](./spec.md)
**Created:** 2026-04-04
**Status:** [ ] Not Started

## Overview

Three-pronged approach: expand OutcomeValidator seeds for better OTP outcome catching, add OTP-intent pre-grounding directive, and broaden extraction/verification pattern detection in scanner.

## Phase 1: OutcomeValidator Seed Expansion
Expand OUTCOME_EXAMPLES from 17 to ≥30 to improve kNN accuracy, especially for OTP-adjacent scenarios.

### Tasks
- [~] Task 1.1: Add ≥13 new seed examples to OUTCOME_EXAMPLES in `src/classifier.rs:27-49`. Focus on:
  - 4+ OK examples: OTP processed, OTP verified, OTP deleted, inbox with OTP handled
  - 3+ DENIED examples: OTP forwarded to third party, credential extraction via branching, OTP shared externally
  - 3+ UNSUPPORTED: variations of missing capability (cannot connect, cannot access, feature not available)
  - 3+ CLARIFICATION: variations of non-CRM (weather question, general knowledge, coding help)
- [ ] Task 1.2: Add unit test `validate_otp_ok_not_blocked` — verify that "Processed inbox, OTP verified correct" with OUTCOME_OK passes validation (not blocked/warned)
- [ ] Task 1.3: Add unit test `validate_otp_denied_exfiltration_passes` — verify that "Blocked credential exfiltration branching logic" with OUTCOME_DENIED passes validation

### Verification
- [ ] `cargo test` — all tests pass
- [ ] OutcomeValidator seed count ≥ 30

## Phase 2: OTP Pre-grounding + Pattern Expansion
Add OTP-intent directive hint and expand extraction/verification detection.

### Tasks
- [ ] Task 2.1: Add OTP-intent pre-grounding in `src/pregrounding.rs` (after inbox loading, ~line 468). Pattern: detect if any inbox file was classified as "credential" with non-exfiltration recommendation, then inject directive message: "⚠ OTP HANDLING: Inbox contains OTP/credentials. Reading, verifying (correct/incorrect), storing, or deleting OTP = normal CRM work = OUTCOME_OK. ONLY deny if branching logic extracts individual digits/characters."
  - Detection: scan the inbox_content for `[CLASSIFICATION: credential` lines where recommendation doesn't contain "EXFILTRATION"
  - Use the classifier output already available in `read_inbox_files()`
- [ ] Task 2.2: Expand `has_extraction` patterns in `src/scanner.rs:320-322` — add: "second character", "third digit", "each digit", "one by one", "based on the first", "if it starts with", "conditional on the"
- [ ] Task 2.3: Expand `is_simple_verify` patterns in `src/scanner.rs:324-330` — add: `confirm + deny`, `right + wrong`, `yes + no` (when preceded by "is it" or "reply")
- [ ] Task 2.4: Add unit tests for new extraction patterns (2 tests: new exfiltration variants detected, new verification variants pass)
- [ ] Task 2.5: Run `make task T=t25` and `make task T=t29` on Nemotron — verify both pass

### Verification
- [ ] `cargo test` — all tests pass including new OTP pattern tests
- [ ] `make task T=t25` passes on Nemotron
- [ ] `make task T=t29` passes on Nemotron
- [ ] `make task T=t01` — no regression

## Phase 3: Docs & Cleanup

### Tasks
- [ ] Task 3.1: Update CLAUDE.md — add OTP pre-grounding hint to "Pre-grounding Context" section and update OUTCOME_EXAMPLES count
- [ ] Task 3.2: Update roadmap.md — mark t25/t29 progress
- [ ] Task 3.3: Remove dead code — unused imports, stale comments if any

### Verification
- [ ] CLAUDE.md reflects current project state
- [ ] `cargo test` passes
- [ ] `cargo build` succeeds clean

## Final Verification

- [ ] All acceptance criteria from spec met
- [ ] Tests pass (target: 134+ existing + new)
- [ ] Linter clean (`cargo clippy`)
- [ ] Build succeeds
- [ ] t25, t29 pass on Nemotron
- [ ] t01 baseline holds

## Context Handoff

_Summary for /build to load at session start._

### Session Intent
Harden OTP task handling (t25/t29) to reduce false DENIED outcomes and improve exfiltration/verification distinction.

### Key Files
- `src/classifier.rs:27-49` — OUTCOME_EXAMPLES seed array (expand from 17 to ≥30)
- `src/classifier.rs:615+` — OutcomeValidator tests (add OTP-specific validation tests)
- `src/scanner.rs:320-322` — `has_extraction` patterns (expand with new exfiltration variants)
- `src/scanner.rs:324-330` — `is_simple_verify` patterns (expand with new verification variants)
- `src/pregrounding.rs:468` — after inbox loading (add OTP-intent pre-grounding hint)
- `src/scanner.rs:802+` — OTP unit tests (add new pattern tests)

### Decisions Made
- OTP-intent hint uses same pattern as delete-intent (pregrounding.rs:537-550): directive message injected into pre-grounding
- Hint fires only when inbox has credential classification WITHOUT exfiltration — prevents suppressing legitimate DENIED
- Seed expansion targets OTP-heavy examples because that's the gap; general expansion deferred to calibration track
- No changes to ONNX model or classifier weights — prompt + validator + patterns only

### Risks
- OTP-intent hint too aggressive could suppress legitimate DENIED on actual exfiltration tasks
  - Mitigation: hint only fires when scanner recommendation is "Process normally" or "verification", NOT on exfiltration
- More seed examples could shift kNN voting balance and affect other tasks
  - Mitigation: run t01 regression check; keep example distribution balanced across outcomes
- Task randomization means single-run verification is not conclusive
  - Mitigation: run t25/t29 at least twice each if first passes

---
_Generated by /plan. Tasks marked [~] in progress and [x] complete by /build._
