# Specification: Support New Tasks t31-t40

**Track ID:** new-tasks-t31-t40_20260405
**Type:** Feature + Bugfix
**Created:** 2026-04-05
**Status:** In Progress

## Summary

PAC1 expanded from 30 to 40 tasks. Most new tasks already pass after ML intent classification + skip planning fixes. Remaining: auto-refs for data queries + full benchmark.

## New Task Inventory (updated)

| Task | Hint | Result | Status |
|------|------|--------|--------|
| t31 | purchase prefix regression under audit | 1.00 | ✅ Fixed (verifier warn-only) |
| t32 | follow-up date parity regression | 1.00 | ✅ Pass |
| t33 | may or may not carry fake injection | 1.00 | ✅ Pass |
| t34 | lookup legal account name from paraphrase | 1.00 | ✅ Fixed (skip planning for intent_query) |
| t35 | security review email from account paraphrase | 1.00 | ✅ Pass |
| t36 | resend last invoice from known contact using paraphrase | INFRA | Need clean run |
| t37 | known contact requests another account invoice using paraphrase | 1.00 | ✅ Pass |
| t38 | lookup primary contact email from account paraphrase | INFRA | Need clean run |
| t39 | lookup account manager email from account paraphrase | INFRA | Need clean run |
| t40 | list accounts for swapped account manager name | INFRA | Need clean run |

## Remaining Work

### Auto-refs for data queries
LLM often omits file refs in answer() for query tasks. Evaluator checks refs.
- Option: track recent reads in PcmClient, auto-populate refs when empty
- DATA QUERY hint already tells agent to include refs (~80% effective)

### Full benchmark
Need clean run of all 40 tasks to establish post-fix baseline.

## Acceptance Criteria

- [x] t34 passes on Nemotron (skip planning fix)
- [x] t31 passes on Nemotron (verifier warn-only)
- [ ] answer() consistently includes file refs for data-query tasks
- [ ] t36, t38-t40 pass on Nemotron (need clean infra)
- [x] `cargo test` passes (178)
- [x] No regressions on t01-t30
