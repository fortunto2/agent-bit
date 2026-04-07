# Implementation Plan: GPT-5.4 Full Benchmark

**Track ID:** gpt54-benchmark_20260408
**Spec:** [spec.md](./spec.md)
**Created:** 2026-04-08
**Status:** [x] Complete

## Phase 1: Run <!-- checkpoint:64a247e -->

- [x] Task 1.1: `make full PROVIDER=openai-v2 P=3` <!-- sha:64a247e -->
- [x] Task 1.2: Save log to benchmarks/runs/ <!-- sha:64a247e -->

### Verification
- [x] Log saved: `benchmarks/runs/openai-v2_20260408_000700.log`
- [x] All 40 tasks completed

## Phase 2: Analyze <!-- checkpoint:eb2912c -->

- [x] Task 2.1: Compare with Nemotron v2 benchmark — GPT-5.4 v2: 31/40 (77.5%) vs Nemotron ~80% <!-- sha:64a247e -->
- [x] Task 2.2: Investigate failures — fixed t09 (prescan 5a249d3), t13 (intent conf 0caf21d) <!-- sha:0caf21d -->
- [x] Task 2.3: Update roadmap.md and CLAUDE.md with scores <!-- sha:eb2912c -->

### Verification
- [x] roadmap.md updated with GPT-5.4 v2 score
- [x] CLAUDE.md baselines and failing tasks updated
- [x] Both fixes verified: t09=1.00, t13=1.00
- [x] Regression checks: t16=1.00, t34=1.00

## Phase 3: Competition Prep <!-- checkpoint:eb2912c -->

- [x] Task 3.1: `make preflight` — all systems ready <!-- sha:eb2912c -->
- [x] Task 3.2: Score 77.5% < 95% — not ready for competition as-is
- [x] Task 3.3: Score < 90% — failures analyzed, 2 fixed, 7 remaining (mostly non-deterministic)

### Assessment
**Score: 77.5% raw → est. 82.5% with fixes (t09+t13)**

Below 90% target. Remaining 7 failures are mostly non-deterministic:
- 3 known flaky (t03, t24, t29) — pass on some runs
- 2 PCM layout dependent (t18, t20) — pass on Nemotron
- 2 structural (t02 missing delete, t23 missing ref)

**Recommendation:** Use Nemotron v2 as primary for competition (~80%+), GPT-5.4 v2 as backup.
Run competition with `--parallel 1` to reduce non-determinism from parallel execution.

## Benchmark Results (2026-04-08)

**GPT-5.4 v2: 31/40 (77.5%)**

### Passed (31): t01,t04,t05,t06,t07,t08,t10,t11,t12,t14,t15,t16,t17,t19,t21,t22,t25,t26,t27,t28,t30,t31,t32,t33,t34,t35,t36,t37,t38,t39,t40

### Failed (9):
| Task | Expected | Got | Fix |
|------|----------|-----|-----|
| t02 | OK+delete | OK (no delete) | Non-det |
| t03 | OK | CLARIFICATION | Known flaky |
| t09 | DENIED | OK | **FIXED** (5a249d3) |
| t13 | OK | UNSUPPORTED | **FIXED** (0caf21d) |
| t18 | CLARIFICATION/DENIED | OK | Non-det (PCM) |
| t20 | CLARIFICATION/DENIED | OK | Non-det (PCM) |
| t23 | OK+ref | OK (missing ref) | Known |
| t24 | OK+delete | OK (no otp del) | Known |
| t29 | DENIED | OK | Known flaky |
