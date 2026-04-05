# PAC1 Agent Roadmap

## Goal: 100% on Nemotron (30/30)

Target: every task passes deterministically. After fixing each task, run full benchmark (`make full`). Repeat until 30/30.

## Current Score
- Nemotron 120B: **80%** (24/30)
- GPT-5.4: ~85% (25-27/30)
- GPT-5.4-mini: 65% (20/31)

## Cost Policy

- **Focus on Nemotron** (free via Cloudflare Workers AI). ALL development and testing on Nemotron.
- **OpenAI (GPT-5.4): ONLY for final validation** — max 1-2 runs per session. Never iterate on OpenAI.
- `make task T=tXX` defaults to Nemotron. Do NOT add PROVIDER=openai unless final check.
- Never `make full PROVIDER=openai-full` — too expensive.

## Process

For each failing task:
1. `/solo:plan "Fix tXX — description"` → create plan in docs/plan/
2. `/solo:build trackId` → execute plan
3. `make task T=tXX` → verify on **Nemotron** (free)
4. `make task T=t01` → regression check on **Nemotron**
5. Archive plan to docs/plan-done/

After ALL tasks fixed:
6. `make full` → full 30-task benchmark on **Nemotron**
7. If any fail → create new plan for that task, repeat from step 1
8. Goal: **3 consecutive full runs at 30/30 on Nemotron**
9. Final validation: ONE run on GPT-5.4 to confirm cross-model

## Failing Tasks

All 6 remaining fails pass on some runs but not consistently.

### Priority 1: Over-cautious (DENIED instead of OK)
- [x] **t19** — FIXED: separate MISMATCH from UNKNOWN in ensemble blocker
- [x] **t23** — "process inbox" — hardened for Nemotron: directive hints, inbox processing guidance, loop threshold 25, auto-answer writes-based OK. Passes ~2/3

### Priority 2: Execution failures
- [~] **t03** — write-nudge counter fix (reads-since-last-write, threshold 2). Verified 1/1 pass on Nemotron.
- [~] **t08** — structural task_type forcing (`detect_forced_task_type`). Needs Nemotron verification.

### Priority 3: OTP handling
- [~] **t25** — "process inbox" (OTP severity — DENIED vs OK) — expanded seeds (32), OTP-intent hint (conf>0.50), extraction patterns (+7), verify patterns (+3). Still non-deterministic.
- [~] **t29** — "process inbox" (OTP verify — exfiltration vs legit check) — same hardening. Scanner `is_simple_verify` false-positives on low-confidence content remain.

## Rules

- **NO hardcoded hacks.** Every fix must be universal — tasks change every run.
- **NO task-ID checks.** If the fix wouldn't work with different wording → it's wrong.
- Prefer: prompt wording > classifier tuning > structural signals > new code
- After each fix: `cargo test` + `make task T=tXX` + `make task T=t01`

## Architecture TODO
- [x] Temperature annealing: planning_temperature=0.4, execution=0.1 (EAD-inspired)
- [x] Decision framework reframing: "DENIED requires EXPLICIT evidence" in system prompt
- [x] Confidence-gated reflection: confidence<0.7 triggers re-evaluation (AUQ-inspired)
- [x] Blocking OutcomeValidator (calibrated: 50 seeds, threshold 0.80 validated, store audited)
- [x] Prompt diet experiment: SYSTEM_PROMPT_EXPLICIT is NOT bloat — all 44 lines load-bearing for Nemotron. PLANNING_PROMPT safely slimmed by 2 patterns.
- [ ] NLI model for zero-shot classification (rust-bert)
- [ ] Gemma 4 26B testing (CF access pending)

## Done
- [x] t23: Nemotron hardening (directive hints, loop threshold, auto-answer, contacts pre-load)
- [x] t19: ensemble blocker MISMATCH/UNKNOWN split
- [x] 13 tasks fixed (t04,t06,t08,t12,t18,t19,t20,t22,t23,t24,t25,t28,t30)
- [x] bitgn-sdk v0.2.0 published (first Rust SDK)
- [x] Full SDK migration (pcm.rs + bitgn.rs)
- [x] schemars, ammonia, mailparse, unicode-normalization
- [x] Adaptive OutcomeValidator, dynamic examples, single prompt
- [x] Session affinity, outbox validation, escaped HTML detection
- [x] Temperature annealing + decision framework + confidence-gated reflection (stabilize-decisions track)
- [x] Write-nudge counter fix + structural task_type forcing (harden-t03-t08 track)
- [x] Prompt diet (2026-04-05): PLANNING_PROMPT slimmed, SYSTEM_PROMPT_EXPLICIT reverted (all content load-bearing)
