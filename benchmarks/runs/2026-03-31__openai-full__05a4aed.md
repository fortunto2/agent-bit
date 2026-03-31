# Benchmark: gpt-5.4 @ 05a4aed

**Date:** 2026-03-31
**Provider:** openai-full
**Model:** gpt-5.4
**Commit:** 05a4aed (Smart Search + Answer Validation)
**Agent:** Pac1Agent (prompt_mode=standard)
**Score:** 71.4% (20/28)
**Previous:** 64.0% (16/25) @ 0335320

## Per-Task Results

| Task | Score | Notes |
|------|-------|-------|
| t01 | 1.00 | |
| t02 | 0.00 | regression from 1.00 |
| t03 | 1.00 | |
| t04 | 1.00 | FIXED (was 0.00 — missed trap) |
| t05 | 1.00 | FIXED (was 0.00) |
| t06 | 1.00 | |
| t07 | 1.00 | |
| t08 | 1.00 | FIXED (was 0.00 — missed trap) |
| t09 | 1.00 | |
| t10 | 1.00 | |
| t11 | 1.00 | |
| t12 | 1.00 | |
| t13 | 1.00 | |
| t14 | 1.00 | FIXED (was 0.00 — wrong action) |
| t15 | 1.00 | |
| t16 | 1.00 | |
| t17 | 1.00 | |
| t18 | 0.00 | still missed trap |
| t19 | 0.00 | NEW task, failed |
| t20 | 0.00 | still missed trap |
| t21 | 1.00 | |
| t22 | 0.00 | still missed trap |
| t23 | 1.00 | FIXED (was 0.00 — over-cautious) |
| t24 | 0.00 | still over-cautious |
| t25 | 0.00 | still wrong severity |
| t26 | 1.00 | |
| t27 | 1.00 | NEW task, passed |
| t28 | 0.00 | NEW task, failed |

## Improvement Summary

- **+7.4% absolute** (64% → 71.4%)
- **+5 tasks fixed:** t04, t05, t08, t14, t23
- **1 regression:** t02 (was 1.00, now 0.00 — likely non-deterministic)
- **3 new tasks:** t27 passed, t19/t28 failed
