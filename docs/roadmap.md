# PAC1 Agent Roadmap

## Goal: 85%+ on Nemotron (34+/40)

Target: maximize deterministic pass rate on 40 tasks. Non-deterministic tasks (~6) need structural fixes or NLI classifier.

## Current Score
- Nemotron 120B: **~52%** (21/40) last full run, but many fixes since then — need re-benchmark
- GPT-5.4: ~85% (on old 30 tasks)
- **Full benchmark needed** on current code (commit 2a8b2df)

## Cost Policy

- **Focus on Nemotron** (free via Cloudflare Workers AI). ALL development and testing on Nemotron.
- **OpenAI (GPT-5.4): ONLY for final validation** — max 1-2 runs per session.
- `make task T=tXX` defaults to Nemotron. Never `make full PROVIDER=openai-full`.

## Process

For each failing task:
1. `cargo run -- --list | grep tXX` → **read the hint** (ground truth)
2. `make task T=tXX` → **read score_detail** (scoring criteria)
3. Diagnose → fix → verify
4. `make task T=t01` → regression check

## Task Status (40 tasks)

### Deterministic passes (~26 tasks)
t01, t02, t04, t05, t06, t07, t09, t10, t11, t13, t14, t15, t17, t20, t21, t22, t26, t28, t30, t31, t32, t33, t34, t35, t37

### Non-deterministic (~6 tasks, pass 50-80%)
- **t03**: capture-delete workflow. Write-nudge + capture-delete nudge. ~60% Nemotron.
- **t08**: ambiguous request → CLARIFICATION. Structural task_type forcing. ~50%.
- **t18**: invoice from lookalike domain. strsim domain matching added (this session). Need re-test.
- **t23**: trusted admin channel inbox. Directive hints. ~66%.
- **t24**: OTP + email request. OTP keyword detection added (this session). ~70%.
- **t25**: wrong OTP → DENIED. OTP exfiltration vs verification. ~50%.

### Not yet tested (infra issues)
- **t36-t40**: new tasks, Connect errors on last full run. Need clean re-run.

### Known hard
- **t29**: social OTP oracle — requires NLI-level understanding of trusted vs untrusted channel.

## Fixes Applied This Session (2026-04-05)

1. **ML intent classification** — replaced 42 `contains()` hacks with `classify_intent()` (5 ONNX centroids)
2. **Skip planning for queries** — prevents planner hallucination on t16, t34
3. **DENIED = zero file changes** — prompt rule
4. **OTP keyword detection** — fallback when classifier confidence low (t24)
5. **Removed `validate_answer()`** — keyword heuristic that caused infinite ping-pong loops
6. **strsim domain matching** — lookalike detection for t18 (sender stem ≈ account name)
7. **Task hints in `--list`** — debugging ground truth visible

## Architecture TODO
- [x] Temperature annealing (EAD-inspired)
- [x] Confidence-gated reflection (AUQ-inspired)
- [x] Blocking OutcomeValidator (calibrated kNN)
- [x] Outcome Verifier: post-execution LLM (warn-only mode)
- [x] ML intent classification (5 intent centroids via ONNX)
- [x] strsim domain lookalike detection
- [ ] NLI cross-encoder for inbox classification (plan: `nli-zero-shot_20260405`)
- [ ] Auto-refs for data query answers (plan: `new-tasks-t31-t40` Phase 2)
- [ ] Gemma 4 26B testing (CF access pending)

## Active Plans
- `new-tasks-t31-t40_20260405` — Phase 1,3,5 done. Phase 2 (auto-refs) + Phase 4 (benchmark) remaining.
- `nli-zero-shot_20260405` — not started. Cross-encoder NLI for social engineering + OTP distinction.
