# Specification: Blocking OutcomeValidator

**Track ID:** blocking-outcome-validator_20260404
**Type:** Feature
**Created:** 2026-04-04
**Status:** Draft

## Summary

The OutcomeValidator currently runs in log-only mode — it predicts the correct outcome using k-NN (k=5, 17 seed + 34 adaptive = 51 examples) but only logs warnings to stderr. The remaining 6 non-deterministic failures (t03, t08, t19, t23, t25, t29) all have prompt/structural fixes in place but still fail intermittently because Nemotron occasionally picks the wrong outcome.

Making the embedding-based validator **blocking** (returning the warning to the model so it can self-correct) directly attacks the non-determinism problem. Conservative confidence gating prevents regressions — block only on near-unanimous k-NN disagreement with high similarity.

## Acceptance Criteria

- [ ] Embedding-based validation is blocking: returns `ToolOutput::text()` when confidence exceeds threshold
- [ ] Confidence gating: blocks only when ≥4/5 votes disagree AND top similarity > 0.80
- [ ] Security-safe: never blocks when chosen outcome is `OUTCOME_DENIED_SECURITY` (trust LLM security decisions)
- [ ] Retry limit: max 1 validation block per trial (second attempt always submits)
- [ ] Score-gated learning: `learn()` re-enabled, gated on trial score ≥ 1.0
- [ ] OutcomeValidator accessible from main.rs for post-trial learning
- [ ] Unit tests for blocking behavior, confidence gating, retry limit, and security exception
- [ ] `cargo test` passes (123+ tests)
- [ ] No regressions on passing tasks (verify with `make task T=t01`)

## Dependencies

- ONNX model files in `models/` (already present)
- Adaptive store `.agent/outcome_store.json` (already 34 entries)
- sgr-agent `ToolOutput::text()` for blocking returns (already used by keyword validation)

## Out of Scope

- Adaptive store cleanup/audit (data quality is acceptable at 51 examples)
- Changing k-NN algorithm or k value
- Adding new seed examples
- NLI model (separate roadmap item)

## Technical Notes

- Keyword validation (tools.rs:542-583) is already blocking — same pattern for embedding validation
- AnswerTool needs `AtomicU32` retry counter to enforce max-1-block policy
- OutcomeValidator needs to be created in main.rs (not pregrounding.rs) for post-trial access
- `learn()` exists and works but is disabled (AI-NOTE comment at tools.rs:645) — re-enable with score gate
- The validation warning text already includes outcome descriptions — model can self-correct from it
- `auto_submit_if_needed()` bypasses the AnswerTool entirely — no validation there (acceptable, it's a fallback)
