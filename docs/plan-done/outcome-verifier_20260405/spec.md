# Specification: Post-Execution Outcome Verifier

**Track ID:** outcome-verifier_20260405
**Type:** Feature
**Created:** 2026-04-05
**Status:** Draft

## Summary

Add a post-execution LLM verification pass that reviews the agent's execution history and confirms or corrects the outcome code before final submission. Currently, the agent decides the outcome inside the execution loop under pressure (multi-step reasoning, tool selection, security assessment). A focused verifier with full execution context — and a much simpler task (4-way classification) — catches outcome errors that cause non-deterministic failures on t08, t25, t29.

The verifier also replaces the fragile heuristic `guess_outcome()` for auto-submit fallback (t03, t23 when agent runs out of steps).

## Acceptance Criteria

- [x] AnswerTool defers submission — stores ProposedAnswer instead of calling pcm.answer() RPC
- [x] New `run_outcome_verifier()` function makes single focused LLM call after execution loop
- [x] Verifier uses structured output (function calling schema) returning outcome + confidence
- [x] Override policy: verifier confidence >= 0.8 AND disagrees → override agent's outcome
- [x] Safety: never override OUTCOME_DENIED_SECURITY (trust security decisions)
- [x] Safety: max 1 override per trial (single verify_and_submit call)
- [x] `guess_outcome()` kept for no-proposed-answer fallback (verifier confused by CRM content)
- [x] Existing unit tests pass (177), no regressions
- [x] Verification adds at most 1 extra LLM call per trial
- [x] t01 passes on Nemotron (Score: 1.00)

## Dependencies

- sgr-agent LlmClient::tools_call() — already available
- Nemotron provider via CF Workers AI — free, no cost concern for extra call

## Out of Scope

- Retry mechanism (re-running agent loop on verification failure) — future track
- Multi-model ensemble (using different model for verification) — future track
- Override for already-submitted answers (RPC doesn't support re-submission)
- Changes to the planning phase or pre-grounding

## Technical Notes

- **Architecture**: Deferred answer pattern. AnswerTool → ProposedAnswer → Verifier → pcm.answer(). The agent loop sees answer() as "done" (answer_submitted=true) but RPC is delayed.
- **Verifier prompt**: Focused 4-way classification. Includes: instruction, execution summary (last 10 ledger entries), proposed answer (if any), classification annotations. Much simpler than the full SYSTEM_PROMPT_EXPLICIT.
- **History truncation**: Verifier gets compact summary (ledger + last_msg), NOT full message history. Keeps verification fast and focused.
- **Weak-model safety**: Verifier prompt uses the same explicit numbered-step style proven to work for Nemotron. Includes examples for each outcome.
- **Override logging**: All verifier decisions logged (agree/disagree/override) for post-run analysis.
- **Reusable code**: `make_llm_config()` in pregrounding.rs already builds LlmConfig — verifier reuses it.
