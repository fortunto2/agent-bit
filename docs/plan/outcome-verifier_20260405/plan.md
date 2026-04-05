# Implementation Plan: Post-Execution Outcome Verifier

**Track ID:** outcome-verifier_20260405
**Spec:** [spec.md](./spec.md)
**Created:** 2026-04-05
**Status:** [ ] Not Started

## Overview

Deferred answer pattern: AnswerTool stores proposed answer → execution loop ends → verifier LLM call reviews history → final submission via pcm.answer(). One extra LLM call per trial, replaces fragile `guess_outcome()` heuristic and catches outcome classification errors.

## Phase 1: Deferred Answer Infrastructure <!-- checkpoint:e1aa184 -->

Add ProposedAnswer storage to PcmClient so AnswerTool can defer RPC submission.

### Tasks
- [x] Task 1.1: Add `ProposedAnswer` struct and `proposed_answer: Mutex<Option<ProposedAnswer>>` to `src/pcm.rs`. Add `propose_answer()` method (stores without RPC) and `submit_answer()` method (reads proposed + calls RPC). Keep existing `answer()` for direct-submit paths (prescan, classifier blocks).
- [x] Task 1.2: Modify `AnswerTool::execute()` in `src/tools.rs` to call `pcm.propose_answer()` instead of `pcm.answer()`. Still sets `answer_submitted=true` so auto_submit doesn't fire. OutcomeValidator validation stays before proposal (blocking still works).
- [x] Task 1.3: Add unit tests for ProposedAnswer flow — propose stores correctly, submit_answer sends RPC, double-propose overwrites.

### Verification
- [x] `cargo test` passes (166 tests)
- [x] AnswerTool no longer calls pcm.answer() directly (only propose_answer)

## Phase 2: Verifier LLM Call <!-- checkpoint:f43e388 -->

Add the verification prompt and `run_outcome_verifier()` function.

### Tasks
- [x] Task 2.1: Add `VERIFIER_PROMPT` to `src/prompts.rs` — focused 4-way classification with numbered steps and examples per outcome. Include explicit Nemotron-friendly verbose style.
- [x] Task 2.2: Add `verify_outcome_tool_def()` to `src/prompts.rs` — function calling schema returning `{outcome, reason, confidence}`.
- [x] Task 2.3: Add `run_outcome_verifier()` to `src/pregrounding.rs` — accepts (LLM config params, instruction, execution_summary, proposed_answer). Builds messages: system=VERIFIER_PROMPT, user=instruction+summary+proposed. Calls `llm.tools_call()` with verify schema. Returns `VerifiedOutcome { outcome, reason, confidence }`. Falls back to proposed answer on LLM error.
- [x] Task 2.4: Unit tests for `verify_outcome_tool_def()` schema validation and response parsing.

### Verification
- [x] `cargo test` passes (169 tests)
- [x] `run_outcome_verifier()` compiles and handles error cases

## Phase 3: Integration & Override Policy <!-- checkpoint:2c70b08 -->

Wire verifier into main execution flow, replace guess_outcome, add override logic.

### Tasks
- [x] Task 3.1: Modify `run_agent()` in `src/pregrounding.rs` — added `build_execution_summary()` helper. Return type unchanged; proposed answer read via `pcm.get_proposed_answer()`.
- [x] Task 3.2: Update `run_trial()` and main loop in `src/main.rs` — after run_trial, call `verify_and_submit()` with verifier + override policy. Log decision as `🔍 Verifier: {agree|override} (conf={})`.
- [x] Task 3.3: Replace `auto_submit_if_needed()` with `verify_and_submit()` — verifier as primary, `guess_outcome()` as fallback when verifier fails or no proposed answer with low confidence.
- [x] Task 3.4: Wire into `run_leaderboard()` — same `verify_and_submit()` call.
- [x] Task 3.5: Add integration-level tests for override policy edge cases: agree, disagree-high-conf, disagree-low-conf, security-never-override, boundary confidence, execution_summary extraction.

### Verification
- [x] `cargo test` passes (176 tests)
- [ ] `make task T=t01` passes on Nemotron (regression check)
- [ ] Verifier logs visible in task output (agree/override)

## Phase 4: Docs & Cleanup

### Tasks
- [x] Task 4.1: Update CLAUDE.md — added Outcome Verifier section, updated architecture, decision pipeline, test count.
- [x] Task 4.2: Update `docs/roadmap.md` — added verifier to Architecture and Done sections.
- [x] Task 4.3: Cleanup — `guess_outcome()` comment updated to reflect fallback-only role, `auto_submit_if_needed` removed, unused `Ordering` import removed.

### Verification
- [x] CLAUDE.md reflects current project state
- [x] Linter clean, tests pass

## Final Verification
- [ ] All acceptance criteria from spec met
- [ ] Tests pass (162+)
- [ ] `cargo build` succeeds with 0 warnings
- [ ] `make task T=t01` passes on Nemotron
- [ ] Verifier logging works end-to-end

## Context Handoff

_Summary for /build to load at session start._

### Session Intent
Add a post-execution LLM verification pass that catches outcome classification errors before submission, improving reliability of non-deterministic tasks.

### Key Files
- `src/pcm.rs` — ProposedAnswer struct, propose_answer(), submit_answer()
- `src/tools.rs` — AnswerTool::execute() → propose instead of submit
- `src/prompts.rs` — VERIFIER_PROMPT, verify_outcome_tool_def()
- `src/pregrounding.rs` — run_outcome_verifier(), return proposed answer from run_agent()
- `src/main.rs` — override policy, replace auto_submit, wire leaderboard

### Decisions Made
- **Deferred answer** over immediate override: can't un-submit via RPC, so defer submission until after verification
- **Function calling** over text completion for verifier: forces structured output, avoids parsing
- **Conservative override** (>= 0.8 confidence): aggressive override would fight the agent; conservative catches only clear errors
- **Never override DENIED_SECURITY**: false negative (missing an attack) is worse than false positive (blocking legit work)
- **Reuse existing LLM config**: same model/provider for verification — no separate model complexity
- **Ledger-based summary** over full history: compact, focused, avoids context overload for verifier

### Risks
- Nemotron may be too weak for reliable verification (same model that made the mistake). Mitigation: verifier task is MUCH simpler (4-way classification vs multi-step reasoning) — success rate should be higher.
- Extra LLM call adds ~5-10s latency per task. Acceptable for 30-task benchmark (~3-5 min total).
- Override could flip a correct answer to wrong. Mitigation: high confidence threshold (0.8) + never override security.

---
_Generated by /plan. Tasks marked [~] in progress and [x] complete by /build._
