# Specification: Calibrate OutcomeValidator — Expand Seeds & Tune Thresholds

**Track ID:** calibrate-outcome-validator_20260404
**Type:** Feature
**Created:** 2026-04-04
**Status:** Draft

## Summary

The OutcomeValidator's blocking mechanism was implemented (blocking-outcome-validator track) but never calibrated. The seed store has 32 static examples — enough for basic voting but insufficient for the specific failure modes of t08 (OK→CLARIFICATION confusion) and t23 (OK→CLARIFICATION on contact ambiguity). The adaptive store has grown to 56 entries organically, but these are unaudited and may contain noise.

The calibration work:
1. Expand seeds from 32 to 50+ with targeted examples for known failure patterns
2. Add confusion-pair seeds — examples that look like one outcome but are actually another (the exact patterns that cause non-deterministic failures)
3. Tune blocking threshold: currently ≥4/5 votes + top_sim > 0.80 is static — consider per-confusion-pair tuning
4. Audit adaptive store quality — remove duplicates and low-quality entries from `.agent/outcome_store.json`

This directly addresses roadmap item: `[ ] Blocking OutcomeValidator (calibrate on 50+ examples, currently at 32 seeds)`.

## Acceptance Criteria

- [ ] Seed store expanded from 32 to 50+ examples in OUTCOME_EXAMPLES (classifier.rs)
- [ ] New seeds cover known failure patterns: delete-task OK (t08), capture-delete OK (t03), multi-contact OK (t23), OTP-verify OK vs OTP-exfiltration DENIED (t25/t29)
- [ ] Confusion-pair seeds added: messages that could be mistaken for wrong outcome (e.g., "not CRM" phrasing in OK-outcome answers, "deleted" in CLARIFICATION-outcome answers)
- [ ] Blocking threshold tuned: empirically tested with `cargo test` validation tests covering all failure patterns
- [ ] Adaptive store audit: script or code to prune duplicates (cosine > 0.95) and rebalance per outcome
- [ ] All 156+ existing tests pass
- [ ] `make task T=t01` baseline passes
- [ ] `make task T=t08` and `make task T=t23` tested (Nemotron, free)

## Dependencies

- `src/classifier.rs` — OUTCOME_EXAMPLES, OutcomeValidator::validate(), blocking threshold
- `.agent/outcome_store.json` — adaptive store (56 entries, audit target)
- `src/tools.rs` — AnswerTool (calls validate, no changes needed)

## Out of Scope

- NLI model (separate roadmap item, higher effort)
- Relaxing security-safe rule (DENIED never blocked — keeping this)
- Prompt changes (no changes to prompts.rs)
- Classifier model retraining (ONNX model stays as-is)

## Technical Notes

### Current state
- 32 seed examples: 10 OK, 7 DENIED, 7 UNSUPPORTED, 6 CLARIFICATION
- 56 adaptive entries: 33 OK, 11 DENIED, 9 UNSUPPORTED, 3 CLARIFICATION
- Total: 88, but seeds are hand-crafted (quality) vs adaptive (organic, noisy)
- Blocking threshold: ≥4/5 votes AND top_sim > 0.80 AND outcome != DENIED
- k=5 nearest neighbors, cosine similarity

### Failure mode analysis (what validator should catch)
- **t08**: Agent picks CLARIFICATION for delete task → validator should block (delete completion = OK)
- **t23**: Agent picks CLARIFICATION for contact ambiguity → validator should block (multi-contact resolution = OK)
- **t25/t29**: Agent picks DENIED for OTP verification → validator CANNOT block (security-safe rule). These tasks need prompt-level fixes only.
- **t03**: Execution failure (agent doesn't complete write+delete) → validator irrelevant (answer not called or auto-answer triggered)

### Seed expansion strategy
- Add 5+ OK examples specifically for: delete completion, capture-delete, inbox multi-message processing, OTP verification, channel data query
- Add 3+ CLARIFICATION examples with edge-case phrasing that avoids false-match with OK patterns
- Add 3+ UNSUPPORTED examples for data-not-found patterns (distinct from CLARIFICATION)
- Total target: 50+ seeds (currently 32, need 18+ new)

### Threshold considerations
- Current 0.80 sim threshold may be too permissive for OK↔CLARIFICATION confusion
- Consider lowering to 0.75 for higher sensitivity, OR adding a secondary check
- Test empirically: run validate() on known failure messages and verify Block/Warn/Pass behavior
