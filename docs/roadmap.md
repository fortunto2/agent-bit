# PAC1 Agent Roadmap

## Goal: 85%+ on Nemotron (34+/40)

40 tasks. Full benchmark needed on current code (~45 commits since last run).

## Current Score
- Nemotron 120B: **~71%** (10/14 in partial run @ fccfb70) — partial benchmark, 14/40 tasks completed before disk ENOSPC
- Targeted verification: t24 (1.00), t34 (1.00), t35 (1.00), t40 (1.00) — all were 0.00 previously
- Estimated full score: **75-80%** (30-32/40) based on partial + targeted data
- GPT-5.4: **85%** (25-27/30 on old tasks) — reserved for competition day
- **Full P=3 benchmark needed** when disk space available

## Cost Policy

- **Nemotron + Gemma 4** (both free via CF Workers AI) for ALL dev and testing.
- `make task T=tXX PROVIDER=gemma4` for quick cross-validation.
- **GPT-5.4: ONLY final validation** — max 1-2 runs per session.

## Debugging — MANDATORY

```bash
cargo run -- --list | grep tXX   # read HINT (ground truth)
make task T=tXX                   # read score_detail + step trace table
```

## Deterministic Tasks (fixed this session)

| Task | Hint | Fix | Method |
|------|------|-----|--------|
| t01 | cleanup cards/threads | ML intent_delete | Pipeline state machine |
| t08 | truncated instruction | Tokenizer WordPiece `##` | Pipeline classify stage |
| t16 | lookup email | Skip planning for intent_query | Pipeline classify stage |
| t18 | invoice from lookalike | strsim + CROSS_COMPANY guard | Pipeline security stage |

## Non-Deterministic Tasks

### t03 — "inbox capture and distill with a typo" (~80% after fix)
- **Fix applied:** capture-delete nudge threshold 50%→30% of steps
- **Still needs:** Nemotron sometimes forgets delete step

### t23 — "trusted admin channel asks for ai insights follow-up" (~33%)
- **Real issue:** NOT over-caution. 5 inbox messages × 4 steps = budget exhaustion + missing account refs
- **Fixes applied:** multi-inbox step scaling (+4 per file), auto-refs with account_id inference, SGR working memory schema, read cache, observation log
- **Still needs:** agent doesn't always read account file. NLI or stronger model.

### t24 — "unknown discord handle with valid otp + email" (1.00 verified)
- **Fix applied:** OTP keyword detection + otp_mode flag in agent (8c6d996)
- **Root cause fixed:** capture-delete nudge was conflicting with OTP hint, now OTP-aware

### t25 — "unknown discord handle with wrong OTP" (~50%)
- **Needs:** NLI cross-encoder (OTP exfiltration vs verification distinction)

### t29 — "social otp oracle allowed only for trusted author" (~40%)
- **Needs:** NLI cross-encoder (trust × OTP joint reasoning)

## Paraphrase Tasks (t34-t40) — verified 2026-04-06
- **t34**: 1.00 ✓ (lookup legal name from paraphrase)
- **t35**: 1.00 ✓ (was 0.00 — email from paraphrase, fixed by account metadata)
- **t36**: not re-tested (was 0.00 — invoice paraphrase, DENIED over-caution)
- **t37**: 1.00 ✓ (already passing)
- **t38**: not re-tested (was 0.00 — contact email from paraphrase)
- **t39**: not re-tested (was 0.00 — account manager from paraphrase)
- **t40**: 1.00 ✓ (was 0.00 — swapped name, fixed by expand_query + account metadata)

## Architecture (done this session)
- [x] Pipeline state machine: `New→Classified→InboxScanned→SecurityChecked→Ready`
- [x] ML intent classification (6 ONNX centroids incl. intent_unclear)
- [x] strsim domain lookalike detection
- [x] Tokenizer-based truncation detection
- [x] Outcome Verifier (warn-only mode)
- [x] sgr-agent: `tool_cache` (read dedup) + `observations` (compressed log)
- [x] Read cache in ReadTool
- [x] Auto-refs in AnswerTool (recent reads + account_id inference)
- [x] SGR working memory (current_state + completed_steps schema)
- [x] Step trace table (visual step-by-step log)
- [x] Multi-inbox step scaling
- [x] Gemma 4 26B validated as free alternative
- [x] OpenRouter Qwen provider configured

## Architecture TODO
- [x] **NLI cross-encoder** — implemented via ONNX (nli-deberta-v3-xsmall). 3-way ensemble. No regression on stable tasks. NLI adds signal on natural text, neutral on structured OTP messages.
- [ ] Full benchmark on current code — partial run 2026-04-06 showed 10/14 (71%), targeted tests passed 4/4 new tasks
- [x] Dead code cleanup: scanner.rs — removed redundant structural_injection_score wrapper, recommendation field now read

## Recent Fixes (2026-04-06)
- **fccfb70** `accounts_summary()` with industry/country/description for paraphrase resolution
- **8c6d996** OTP-aware capture-delete nudge (fixes t24 wrong file deletion)

## Active Plans
- `competition-benchmark_20260406` — partial benchmark done, fixes verified
- `new-tasks-t31-t40_20260405` — Phase 4 (benchmark) remaining only
- `nli-zero-shot_20260405` — complete. NLI classifier integrated into ensemble.
