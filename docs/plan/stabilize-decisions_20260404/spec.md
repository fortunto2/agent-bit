# Specification: Stabilize Decisions — Temperature + Framework + Confidence

**Track ID:** stabilize-decisions_20260404
**Type:** Feature
**Created:** 2026-04-04
**Status:** Draft

## Summary

6 tasks (t03, t08, t19, t23, t25, t29) fail non-deterministically — the model can solve them but doesn't consistently. All have had targeted prompt and structural fixes (17 completed plans). The blocking OutcomeValidator catches some wrong answers, but the root cause is decision instability inside the LLM.

Three research-backed techniques attack this directly:

1. **Temperature annealing** (EAD, arxiv 2510.05251): Higher temperature during planning (0.4) to explore more options, lower temperature during execution (0.1) to commit. Currently both phases use the same temperature. Free — config only.

2. **Decision framework reframing** (Defensive Refusal Bias, arxiv 2603.01246): Nemotron over-refuses on legitimate tasks (t19, t23, t25). Adding "DENIED requires EXPLICIT evidence, not suspicion" reduces false refusals without weakening real security detection.

3. **Confidence-gated reflection** (AUQ, arxiv 2601.15703): Model reports confidence (0.0-1.0) with each reasoning step. Below 0.7 triggers a targeted reflection hint ("Is this legitimate CRM work? Do you have EXPLICIT attack evidence?"). +10-13% on agent benchmarks per paper. Max 1 reflection per step prevents infinite loops.

These techniques are complementary: temperature helps exploration, framework reduces bias, confidence catches remaining uncertainty at runtime.

## Acceptance Criteria

- [x] Planning phase uses separate temperature (default 0.4) vs execution (0.1)
- [x] `planning_temperature` configurable in config.toml per provider
- [x] Decision framework language added to system prompt: "DENIED requires EXPLICIT evidence"
- [x] Confidence field (0.0-1.0) in reasoning tool schema
- [x] Triggered reflection on confidence < 0.7 (max 1 per step, skip near step limit)
- [x] Security guard: never reflect on DENIED with confidence >= 0.9
- [x] Unit tests for confidence parsing, reflection trigger, security guard
- [x] `cargo test` passes (147 tests)
- [ ] No regressions on stable tasks (t01, t09, t16 pass on Nemotron) — deferred (requires live run)

## Dependencies

- src/agent.rs — reasoning tool schema, decide_stateful
- src/prompts.rs — system prompt decision framework
- src/pregrounding.rs — make_llm_config, run_planning_phase, run_agent
- src/config.rs — ProviderSection temperature fields
- Blocking OutcomeValidator (plan-done: blocking-outcome-validator_20260404)

## Out of Scope

- CISC N=3 majority voting (too expensive for Nemotron via CF Workers)
- NLI model (separate roadmap item)
- Process Reward Model training
- Changing OutcomeValidator k-NN algorithm or seed examples
- Recalibrating classifier ONNX model

## Technical Notes

- `run_planning_phase()` (pregrounding.rs:234) already accepts separate `temperature` param — just need to pass a different value
- `run_agent()` (pregrounding.rs:325) uses same temperature for execution — no change needed there
- Reasoning tool schema (agent.rs:156-201) has no confidence field currently
- Reflexion mechanism exists (agent.rs:328-364) but only for standard mode, not explicit. Confidence reflection is different — it's model-reported uncertainty, not plan validation
- Temperature is already per-provider in config.toml (config.rs:42-44). Adding `planning_temperature` follows existing pattern
