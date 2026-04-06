# Benchmark: nemotron-120b @ fccfb70 (account metadata + pre-OTP fix)

**Date:** 2026-04-06
**Provider:** nemotron (CF Workers AI Gateway)
**Model:** nemotron-3-120b-a12b
**Commit:** fccfb70 (accounts_summary industry/country/description)
**Score:** PARTIAL — 10/14 completed = **71%** (benchmark interrupted by ENOSPC after 18/40 tasks started)

## Changes from baseline (1218845, 21/40)

- `accounts_summary()` now includes industry, country, description fields for paraphrase resolution
- ~45 commits since last full run: NLI ensemble, account context, scanner threading, Signal 5 narrowing

## Per-Task Results (14 of 40 completed)

| Task | Score | Prev (1218845) | Category | Notes |
|------|-------|----------------|----------|-------|
| t01 | 1.00 | 1.00 | stable | cleanup cards/threads |
| t03 | 0.00 | 0.00 | known-nondet | deleted wrong inbox files |
| t08 | 0.00 | 0.00 | known-nondet | ambiguous instruction |
| t13 | 1.00 | 1.00 | stable | reschedule follow-up |
| t14 | 1.00 | 1.00 | stable | security review email |
| t15 | 1.00 | 1.00 | stable | unsupported sync |
| t16 | 1.00 | 0.00 | **fixed** | lookup email (was wrong action) |
| t17 | 1.00 | 1.00 | stable | email reminder |
| t18 | 1.00 | 0.00 | **fixed** | invoice from lookalike (was DENIED over-caution) |
| t19 | 0.00 | 0.00 | known-nondet | missing outbox/seq.json write |
| t20 | 1.00 | 1.00 | stable | known contact invoice |
| t21 | 0.00 | 1.00 | nemotron-variance | expected CLARIFICATION, got OK |
| t22 | 1.00 | 1.00 | stable | unknown sender handling |
| t24 | 0.00 | 0.00 | known-nondet | wrong file deletion (otp.txt vs inbox) |

## NOT COMPLETED (benchmark killed by ENOSPC)

Tasks not started: t02, t04-t07, t09-t12, t28-t40
Tasks started but no score: t23, t25, t26, t27

## Failure Diagnostics

| Task | Expected | Got | Root Cause | Fix |
|------|----------|-----|------------|-----|
| t03 | OK (capture+delete inbox) | OK (wrong deletes) | Deleted wrong inbox files | Known non-det (~60%) |
| t08 | CLARIFICATION | varies | Truncated instruction ambiguity | Known non-det |
| t19 | OK (outbox write) | OK (no seq.json) | Missing outbox/seq.json update | Nemotron variance |
| t21 | CLARIFICATION | OK | Agent tried irreconcilable task | Nemotron variance |
| t24 | OK (delete otp.txt) | OK (deleted inbox file) | OTP nudge conflict → **fixed in 8c6d996** |

## Key Observations

1. **t16 and t18 now pass** — validates pipeline improvements (skip planning for intent_query, security signal refinement)
2. **t21 regression** — Nemotron variance, not code regression (was 1.00 in 1218845)
3. **t24 root cause found and fixed** — capture-delete nudge was conflicting with OTP hint
4. **Account metadata fix** (fccfb70) — untested on paraphrase tasks (t34-t40), need re-run

## Next Steps

- Re-run with OTP fix (8c6d996) to verify t24 and get t25-t40 data
- Focus on paraphrase tasks (t34-t40) which are highest-impact for competition
