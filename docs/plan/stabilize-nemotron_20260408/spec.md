# Specification: Stabilize Nemotron remaining fails

**Track ID:** stabilize-nemotron_20260408
**Type:** Bug
**Created:** 2026-04-08

## Summary

Remaining Nemotron fails after workflow SM + hooks + policy refactor:
- t19: KNOWN sender + invoice → DENIED (model hallucinate "injection")
- t21: non-CRM content → DENIED instead of CLARIFICATION
- t23: 5 inbox multi-step → read-loop, never writes (step budget)
- t29: OTP oracle → trial-dependent (handle trust parsing)

All pass on GPT-5.4. Root cause: Nemotron model limitations.

## Acceptance Criteria

- [ ] Try each task 3x on Nemotron, investigate failing runs via BitGN logs
- [ ] For each: identify if structural fix possible or model limitation
- [ ] Any fix must not regress t01, t03, t05, t24, t27
- [ ] Document findings in plan for future sessions
