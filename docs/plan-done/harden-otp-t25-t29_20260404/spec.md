# Specification: Harden OTP Handling (t25/t29)

**Track ID:** harden-otp-t25-t29_20260404
**Type:** Bug
**Created:** 2026-04-04
**Status:** Draft

## Summary

Tasks t25 and t29 fail non-deterministically despite the fix-otp-classification track. t25 ("process inbox" with OTP) sometimes gets false DENIED when the OTP should be processed normally. t29 ("OTP verify") confuses exfiltration with verification — only 0.50 (2/4) on GPT-5.4 evolve runs. The root cause is the LLM (especially Nemotron) not reliably following the OTP decision tree, compounded by insufficient OutcomeValidator coverage for OTP scenarios (only 17 seed examples, target is 50+).

Three-pronged fix: (1) OTP-intent pre-grounding directive hint to prevent false DENIED at the LLM level, (2) expanded OutcomeValidator seed examples to catch wrong outcomes, (3) broader extraction pattern detection to reduce ambiguity in scanner recommendations.

## Acceptance Criteria

- [x] OTP-intent pre-grounding hint injected when inbox has credential-classified content (mirrors delete-intent pattern at pregrounding.rs:537)
- [x] OutcomeValidator OUTCOME_EXAMPLES expanded from 17 to ≥30 with OTP-specific entries
- [x] Scanner `has_extraction` patterns expanded to catch more exfiltration variants (≥3 new patterns)
- [x] Scanner `is_simple_verify` patterns expanded to catch more verification variants (≥2 new patterns)
- [ ] t25 passes on Nemotron (`make task T=t25`) — non-deterministic, DENIED-expected variants
- [ ] t29 passes on Nemotron (`make task T=t29`) — non-deterministic, DENIED-expected variants
- [x] All existing 134 tests pass + new tests added
- [x] No regressions on t01 baseline (`make task T=t01`)

## Dependencies

- fix-otp-classification (plan-done) — this track builds on its work
- blocking-outcome-validator_20260404 (plan-done) — validator infrastructure already in place

## Out of Scope

- Calibrating OutcomeValidator to full 50+ examples across ALL outcome types (this track focuses on OTP-heavy seeds)
- Changing the ONNX model or retraining the classifier
- NLI model for zero-shot classification (separate Architecture TODO)

## Technical Notes

- Delete-intent pre-grounding pattern (pregrounding.rs:537-550) is the template for OTP-intent hint
- Current seed examples: 17 (6 OK, 4 DENIED, 4 UNSUPPORTED, 3 CLARIFICATION)
- Current adaptive store: 41 examples (21 OK, 11 DENIED, 7 UNSUPPORTED, 2 CLARIFICATION) — skewed toward OK
- Scanner `has_extraction` patterns (scanner.rs:320-322): "first character", "first digit", "depending on", "branch", "character of", "digit of", "if the code" — 7 patterns
- Scanner `is_simple_verify` patterns (scanner.rs:324-330): correct+incorrect, valid+invalid, match+doesn't match, verify, check+correct — 5 patterns
- OTP pre-grounding hint should fire ONLY when classifier says "credential" AND recommendation is "Process normally" or "verification" — not when exfiltration is detected
