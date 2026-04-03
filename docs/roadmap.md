# PAC1 Agent Roadmap

## Goal: 100% on Nemotron (30/30)

Target: every task passes deterministically. After fixing each task, run full benchmark (`make full`). Repeat until 30/30.

## Current Score
- Nemotron 120B: **80%** (24/30)
- GPT-5.4: ~85% (25-27/30)
- GPT-5.4-mini: 65% (20/31)

## Process

For each failing task:
1. `/solo:plan "Fix tXX — description"` → create plan in docs/plan/
2. `/solo:build trackId` → execute plan
3. `make task T=tXX` → verify fix
4. `make task T=t01` → regression check
5. Archive plan to docs/plan-done/

After ALL tasks fixed:
6. `make full` → full 30-task benchmark on Nemotron
7. If any fail → create new plan for that task, repeat from step 1
8. Goal: **3 consecutive full runs at 30/30** (confirms determinism)

## Failing Tasks

All 6 remaining fails pass on some runs but not consistently.

### Priority 1: Over-cautious (DENIED instead of OK)
- [x] **t19** — FIXED: separate MISMATCH from UNKNOWN in ensemble blocker
- [~] **t23** — "process inbox" — contact pre-grounding + search annotation implemented, needs harness verification

### Priority 2: Execution failures
- [ ] **t03** — "capture from inbox, distill, delete" (Nemotron misses file ops)
- [ ] **t08** — "delete that card" (ambiguous task → model makes unexpected changes)

### Priority 3: OTP handling
- [ ] **t25** — "process inbox" (OTP severity — DENIED vs OK)
- [ ] **t29** — "process inbox" (OTP verify — exfiltration vs legit check)

## Rules

- **NO hardcoded hacks.** Every fix must be universal — tasks change every run.
- **NO task-ID checks.** If the fix wouldn't work with different wording → it's wrong.
- Prefer: prompt wording > classifier tuning > structural signals > new code
- After each fix: `cargo test` + `make task T=tXX` + `make task T=t01`

## Architecture TODO
- [ ] Blocking OutcomeValidator (calibrate on 50+ examples)
- [ ] NLI model for zero-shot classification (rust-bert)
- [ ] Gemma 4 26B testing (CF access pending)

## Done
- [x] t19: ensemble blocker MISMATCH/UNKNOWN split
- [x] 13 tasks fixed (t04,t06,t08,t12,t18,t19,t20,t22,t23,t24,t25,t28,t30)
- [x] bitgn-sdk v0.2.0 published (first Rust SDK)
- [x] Full SDK migration (pcm.rs + bitgn.rs)
- [x] schemars, ammonia, mailparse, unicode-normalization
- [x] Adaptive OutcomeValidator, dynamic examples, single prompt
- [x] Session affinity, outbox validation, escaped HTML detection
