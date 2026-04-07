# PAC1 Agent Roadmap

## Goal: 85%+ on GPT-5.4, 80%+ on Nemotron

Competition: April 11, 2026 (13:00-15:00 CEST)

## Current Scores (2026-04-08)

| Model | Prompt | Score | Notes |
|-------|--------|-------|-------|
| GPT-5.4 | v2 | **77.5% (31/40)** | Full benchmark 2026-04-08. With fixes: est. 82.5% |
| Nemotron | v2 | ~80%+ (est) | Baseline partial benchmark 2026-04-07 |
| Nemotron | explicit | 75% (30/40) | Full benchmark 2026-04-06 |
| GPT-5.4 | explicit | ~85% (est) | Historical estimate |

## Fixes Applied (2026-04-08)

| Fix | Impact | Commit |
|-----|--------|--------|
| Verifier selective security override | t09: 0→1 (catches injection agent misses) | 64a247e |
| Prescan: "BEGIN TRUSTED PATCH" detection | t09: structural block | 5a249d3 |
| Intent confidence-gate planning skip | t13: 0→1 (low-conf query no longer skips planning) | 0caf21d |

## Still Failing

| Task | GPT-5.4 | Nemotron | Root cause |
|------|---------|---------|-----------|
| t02 | ❌ (no delete) | ? | Agent misses thread delete step |
| t03 | ❌ | ~60% | Capture-delete non-deterministic |
| t18 | ❌ | ✅ | inbox_files=0 on some trials (PCM layout) |
| t20 | ❌ | ? | Cross-account not detected (inbox layout) |
| t23 | ❌ | ❌ | 5 inbox, step budget, missing refs |
| t24 | ❌ (no otp del) | ✅ | OTP cleanup not triggered |
| t29 | ❌ | ~50% | OTP oracle trust — trial-dependent |

## Infrastructure Done

- [x] V2 annotation-driven prompt
- [x] Read cache in PcmClient (auto-invalidation on write/delete)
- [x] Pac1SgrAgent (pure SGR, 4x faster, experimental)
- [x] Trial dump system (PCM files + BitGN log URL)
- [x] NLI cross-encoder (3-way ensemble)
- [x] Pipeline state machine with structural guards
- [x] Verifier selective security override (v0.4)
- [x] Intent confidence threading through pipeline
- [x] Prescan: fake authority blocks detection
- [x] dotenvy auto-load .env

## Competition Prep (April 11)

1. ~~Full GPT-5.4 benchmark to confirm score~~ Done: 77.5% + 2 fixes
2. `make competition` → warmup Nemotron + scored GPT-5.4 run
3. `make preflight` → verify env, models, store
4. **Decision: use GPT-5.4 v2 as primary** (better reasoning, handles edge cases)
