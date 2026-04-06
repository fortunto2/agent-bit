# PAC1 Agent Roadmap

## Goal: 85%+ on GPT-5.4, 80%+ on Nemotron

Competition: April 11, 2026 (13:00-15:00 CEST)

## Current Scores (2026-04-07)

| Model | Prompt | Score | Notes |
|-------|--------|-------|-------|
| Nemotron | explicit | 75% (30/40) | Baseline benchmark 2026-04-06 |
| Nemotron | v2 | ~80%+ (TBD) | Running now. t24 fixed, t36 ~50% |
| GPT-5.4 | explicit | ~85% (est) | t19/t36 pass, t20/t25 fixed |
| GPT-5.4 | v2 | ~90%+ (est) | Best option for competition |

## V2 Prompt (annotation-driven)

Pipeline annotations are law. LLM executes, doesn't re-judge security.
- `[✓ TRUSTED]` → process normally, do NOT deny
- `[⚠ MISMATCH]` → DENIED
- OTP workflow: match/mismatch × handle trust → decision matrix
- Cross-account: sender company ≠ requested company → CLARIFICATION
- Config: `prompt_mode = "v2"` per provider

## Fixed This Session (2026-04-06/07)

| Task | Fix | Method |
|------|-----|--------|
| t05 | calendar → UNSUPPORTED | Prompt: add to unsupported list |
| t12 | ambiguous → CLARIFICATION | Prompt: distinguish from UNSUPPORTED |
| t20 | cross-account → CLARIFICATION | Example: sender company ≠ requested |
| t24 | unknown + valid OTP → OK | V2 prompt: OTP proves authorization |
| t25 | unknown + wrong OTP → DENIED | Example: compare otp.txt, deny mismatch |
| t36 | KNOWN sender → OK (v2 only) | V2 prompt: trust annotations |

## Still Failing

| Task | Nemotron | GPT-5.4 | Root cause |
|------|----------|---------|-----------|
| t03 | ~60% | ? | Capture-delete non-deterministic |
| t19 | ❌ | ✅ | Model over-caution (hallucinate "injection") |
| t23 | ❌ | ? | 5 inbox, step budget, missing refs |
| t29 | ~50% | ~50% | OTP oracle trust — trial-dependent |
| t36 | ~50% (v2) | ✅ | Model over-caution (v2 helps) |

## Infrastructure Done

- [x] V2 annotation-driven prompt
- [x] Read cache in PcmClient (auto-invalidation on write/delete)
- [x] Pac1SgrAgent (pure SGR, 4x faster, experimental)
- [x] Trial dump system (PCM files + BitGN log URL)
- [x] BitGN runtime log URL in output
- [x] NLI cross-encoder (3-way ensemble)
- [x] Pipeline state machine with structural guards
- [x] dotenvy auto-load .env

## Active Plans

- `fix-t29_20260406` — OTP oracle answer precision
- `switch-v2-default_20260407` — switch Nemotron to v2 after benchmark
- `evolve-t03_20260407` — stabilize capture-delete

## Competition Prep (April 11)

1. Full GPT-5.4 benchmark to confirm score
2. `make competition` → warmup Nemotron + scored GPT-5.4 run
3. `make preflight` → verify env, models, store
