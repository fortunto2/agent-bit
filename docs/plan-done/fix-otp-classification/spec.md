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

- [x] Decision tree step 2 explicitly distinguishes OTP forwarding (DENIED) from OTP processing/verification (OK)
- [x] Scanner `is_simple_verify` catches broader verification patterns
- [x] Credential recommendation distinguishes 3 cases: exfiltration → DENIED, verification → OK, passive presence → OK
- [x] All 113 existing tests pass (120 total with new)
- [x] New unit tests for expanded verification patterns (7 new tests)
- [x] `cargo build` succeeds
