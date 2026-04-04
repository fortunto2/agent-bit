# Implementation Plan: Blocking OutcomeValidator

**Track ID:** blocking-outcome-validator_20260404
**Spec:** [spec.md](./spec.md)
**Created:** 2026-04-04
**Status:** [x] Complete

## Overview

Convert the embedding-based OutcomeValidator from log-only to confidence-gated blocking mode. Add retry limit to prevent infinite loops. Move validator creation to main.rs so post-trial score-gated learning can work. Conservative thresholds: ≥4/5 votes + top_sim > 0.80 + not DENIED.

## Phase 1: Confidence-Gated Blocking <!-- checkpoint:9e8ef11 -->

Make the embedding validator return warnings to the model (like keyword validation already does) but only when confidence is high enough to avoid regressions.

### Tasks

- [x] Task 1.1: Add `ValidationMode` enum <!-- sha:9e8ef11 --> to `src/classifier.rs` — `Block(String)` vs `Warn(String)` vs `Pass`. Modify `validate()` to return `ValidationMode` instead of `Option<String>`. Block when `pred_votes >= 4 && top_sim > 0.80 && outcome != "OUTCOME_DENIED_SECURITY"`. Warn (log-only) for 3/5 votes. Pass otherwise.
- [x] Task 1.2: Add `validation_retries: AtomicU32` <!-- sha:9e8ef11 --> field to `AnswerTool` in `src/tools.rs`. Increment on each block. If retries >= 1, skip embedding validation and submit (max 1 block per trial).
- [x] Task 1.3: Update `AnswerTool::execute()` <!-- sha:9e8ef11 --> in `src/tools.rs:637-642` — match on `ValidationMode::Block` to return `ToolOutput::text()`, `Warn` to only `eprintln!`, `Pass` to proceed. Check retry counter before calling validate.
- [x] Task 1.4: Unit tests <!-- sha:9e8ef11 --> in `src/classifier.rs` — test `validate()` returns Block for 4/5 unanimous high-sim, Warn for 3/5, Pass for agreement. Test DENIED exception (never blocks when chosen is DENIED_SECURITY).

### Verification

- [x] `cargo test` passes (128 tests)
- [x] `cargo build` clean (no warnings from pac1-agent)

## Phase 2: Score-Gated Learning <!-- checkpoint:fa53ad3 -->

Re-enable adaptive learning but only for confirmed correct answers (trial score ≥ 1.0).

### Tasks

- [x] Task 2.1: Move `OutcomeValidator` creation <!-- sha:fa53ad3 --> from `src/pregrounding.rs:496-507` to `src/main.rs` — create it once per run in `run_playground()`/`run_leaderboard()`, pass as `Option<Arc<OutcomeValidator>>` to `run_trial()` → `run_agent()`. Update `run_agent()` signature in `src/pregrounding.rs` to accept `Option<Arc<OutcomeValidator>>`.
- [x] Task 2.2: Add `last_answer: Mutex<Option<(String, String)>>` <!-- sha:fa53ad3 --> field to `OutcomeValidator` in `src/classifier.rs`. Add `store_answer()` and `learn_last()` methods. `store_answer(msg, outcome)` saves to mutex. `learn_last()` calls `learn()` with stored values.
- [x] Task 2.3: Call `validator.store_answer()` <!-- sha:fa53ad3 --> in `AnswerTool::execute()` before `pcm.answer()` submission. Call `validator.learn_last()` in `src/main.rs` after `end_trial()` when score ≥ 1.0.
- [x] Task 2.4: Unit tests — `store_answer()` <!-- sha:fa53ad3 --> stores values, `learn_last()` calls learn with stored values, learning is skipped when no stored answer.

### Verification

- [x] `cargo test` passes (131 tests)
- [x] Adaptive store grows only on successful trials (score-gated via learn_last)

## Phase 3: Verification & Tuning <!-- checkpoint:d7cba70 -->

### Tasks

- [x] Task 3.1: Run `make task T=t01` — score 1.00, no regression — regression check on a passing task. Should still score 1.0.
- [x] Task 3.2: Run `make task T=t08` and `make task T=t25` — both failed but LLM never picked wrong outcome text (no validator trigger) — check if blocking validator improves non-deterministic tasks. Look for "VALIDATION BLOCKED" in stderr.
- [x] Task 3.3: Thresholds kept at ≥4/5 + 0.80 (conservative). Failures are LLM reasoning, not validator scope. (require 5/5 votes or top_sim > 0.85). If no blocks triggered, loosen threshold (3/5 votes or top_sim > 0.75).

### Verification

- [x] t01 passes (1.00, no regression)
- [x] Validator operational (score-gated learning confirmed on t01). No blocks on t08/t25 because LLM answer text matches chosen outcome — validator correctly detects no inconsistency.

## Phase 4: Docs & Cleanup

### Tasks

- [x] Task 4.1: Update CLAUDE.md — document blocking validator behavior, thresholds, retry limit
- [x] Task 4.2: Remove `AI-NOTE: learn() disabled` — already removed in Phase 2 comment from `src/tools.rs:645`
- [x] Task 4.3: `#[allow(dead_code)]` already removed from OutcomeValidator struct/impl in Phase 2. Remaining allows are for CLASS_DESCRIPTIONS and l2_normalize (still unused).

### Verification

- [x] CLAUDE.md reflects current project state
- [x] `cargo test` passes (131), `cargo build` clean

## Final Verification

- [x] All acceptance criteria from spec met
- [x] Tests pass (131)
- [x] Build succeeds (no warnings from pac1-agent)
- [x] Documentation up to date

## Context Handoff

_Summary for /build to load at session start._

### Session Intent

Make the OutcomeValidator block wrong outcomes before submission, with conservative confidence gating and score-gated learning.

### Key Files

- `src/classifier.rs` — OutcomeValidator struct, validate(), learn(), ValidationMode enum
- `src/tools.rs` — AnswerTool struct (retry counter), execute() (blocking logic)
- `src/pregrounding.rs` — run_agent() signature change (accept OutcomeValidator)
- `src/main.rs` — OutcomeValidator creation, post-trial learn_last()

### Decisions Made

- **Block threshold ≥4/5 + sim>0.80**: conservative to prevent regressions. 3/5 is warn-only.
- **Never block DENIED_SECURITY**: trust LLM security decisions — false positives on security are worse than missed completions.
- **Max 1 block per trial**: prevents infinite validation loops. If model can't self-correct after 1 hint, submit anyway.
- **Move validator to main.rs**: required for score-gated learning (main.rs has the trial score, pregrounding.rs doesn't).
- **learn_last() pattern**: store answer in mutex during tool execution, learn from main.rs after trial — avoids plumbing score back through the async chain.

### Risks

- **False positive blocking**: if validator incorrectly blocks a correct answer, model may retry with worse answer. Mitigated by conservative threshold and max-1-retry.
- **Adaptive store poisoning**: learn() was disabled because wrong answers were being learned. Score-gating (≥1.0 only) should prevent this, but monitor store growth.
- **Signature change cascade**: moving OutcomeValidator creation to main.rs changes run_trial() and run_agent() signatures — touch 3 files. Keep backward compatible (Option type).

---

_Generated by /plan. Tasks marked [~] in progress and [x] complete by /build._
