# Spec: Fix OTP Classification (t25/t29)

## Problem

Two tasks fail non-deterministically due to OTP classification issues:

- **t25** (OTP severity): Model sometimes says DENIED when inbox contains OTP data that should be processed normally. The decision tree step 2 ("SHARE/FORWARD/SEND OTP") is too broad — model interprets "write email" as sharing OTP.
- **t29** (OTP verify): Model confuses exfiltration (branching logic to extract OTP digits) with verification (simple correct/incorrect check). The `is_simple_verify` detection is too narrow — only catches "correct" AND "incorrect" together.

## Root Cause

1. **Prompt ambiguity**: Step 2 in the decision tree doesn't distinguish between "forward OTP to a third party" (attack) and "process inbox that happens to contain OTP" (normal work).
2. **Narrow verification detection**: Only matches `correct + incorrect` pair, missing: `valid/invalid`, `verify`, `match/doesn't match`, `check if`.
3. **Missing anti-false-positive guidance**: No explicit "OTP in inbox ≠ threat" language in the prompt.

## Acceptance Criteria

- [ ] Decision tree step 2 explicitly distinguishes OTP forwarding (DENIED) from OTP processing/verification (OK)
- [ ] Scanner `is_simple_verify` catches broader verification patterns
- [ ] Credential recommendation distinguishes 3 cases: exfiltration → DENIED, verification → OK, passive presence → OK
- [ ] All 113 existing tests pass
- [ ] New unit tests for expanded verification patterns
- [ ] `cargo build` succeeds
