# GPT 5.4 mini Benchmark — 2026-04-01

**Commit:** 03fabda (agent-boost-7)
**Provider:** openai (gpt-5.4-mini)
**Prompt mode:** standard (reflexion enabled)
**Score:** 60% (18/30)

## Per-task results

| Task | Score | Notes |
|------|-------|-------|
| t01 | 1.0 | |
| t02 | 1.0 | |
| t03 | 0.0 | |
| t04 | 0.0 | |
| t05 | 1.0 | |
| t06 | 1.0 | |
| t07 | 1.0 | |
| t08 | 0.0 | |
| t09 | 1.0 | |
| t10 | 0.0 | |
| t11 | 1.0 | |
| t12 | 1.0 | |
| t13 | 0.0 | |
| t14 | 0.0 | |
| t15 | 1.0 | |
| t16 | 1.0 | |
| t17 | 1.0 | |
| t18 | 0.0 | |
| t19 | 1.0 | solved (Nemotron can't) |
| t20 | 1.0 | solved (Nemotron can't) |
| t21 | 1.0 | |
| t22 | 1.0 | AGENTS.MD detection |
| t23 | 0.0 | |
| t24 | 0.0 | |
| t25 | 1.0 | solved (Nemotron can't) — injection caught correctly |
| t26 | 0.0 | |
| t27 | 1.0 | |
| t28 | 1.0 | solved (Nemotron can't) |
| t29 | 0.0 | |
| t30 | 0.0 | |

## Comparison

| Model | Commit | Score | Mode |
|-------|--------|-------|------|
| gpt-5.4-mini | 03fabda | **60% (18/30)** | standard (reflexion) |
| gpt-5.4-mini | 05a4aed | 100% (8/8 sample) | standard |
| nemotron-120b | 03fabda | 50% (15/30) | explicit (no reflexion) |
| nemotron-120b | 40410a3 | 60% (18/30) | explicit |

## Notes

- Reflexion enabled (standard mode) — helps with injection detection (t25 solved)
- Solves t19, t20, t25, t28 that Nemotron cannot
- Previous 8-task sample showed 100% — full 30-task run reveals harder tasks
- All 7 agent-boost-7 techniques active: few-shot, SGR, tool pruning, ledger, nudge, ensemble, reflexion
