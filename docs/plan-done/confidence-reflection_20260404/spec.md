# Specification: Confidence-Gated Reflection + Non-Determinism Reduction

**Track ID:** confidence-reflection_20260404
**Type:** Feature
**Created:** 2026-04-04
**Status:** Draft

## Summary

Implement 5 research-backed techniques to reduce non-deterministic failures from 80% to 90%+ on Nemotron. Current 6 failing tasks all pass sometimes — the model is capable but inconsistent. Techniques: temperature annealing, decision framework reframing, confidence field in CoT with triggered reflection, validator retry loop, and optional N=3 voting for persistent flakes.

Sources: AUQ (arxiv 2601.15703), CISC (arxiv 2502.06233), EAD (arxiv 2510.05251), RASC (NAACL 2025), Defensive Refusal Bias (arxiv 2603.01246).

## Acceptance Criteria

- [ ] Temperature split: reasoning steps use 0.3-0.5, answer step uses 0.1
- [ ] Decision framework added to system prompt: DENIED only for EXPLICIT override instructions
- [ ] Confidence field (0.0-1.0) added to reasoning tool schema in agent.rs
- [ ] Triggered reflection: if confidence < 0.7, inject correction hint + re-reason (max 1 retry)
- [ ] Validator retry: if OutcomeValidator disagrees, return correction hint to model (1 retry max)
- [ ] `make full` on Nemotron: score >= 26/30 (87%+)
- [ ] No regression on previously stable tasks (t01, t09, t16 must pass)
- [ ] All tests pass

## Dependencies

- src/agent.rs — reasoning tool schema, decide_stateful
- src/prompts.rs — system prompt decision framework
- src/pregrounding.rs — make_llm_config (temperature)
- src/tools.rs — AnswerTool validator
- src/classifier.rs — OutcomeValidator

## Out of Scope

- CISC N=3 voting (implement only if simpler techniques insufficient)
- NLI model integration
- Process Reward Model training
- Constitutional AI critique chains

## Technical Notes

### Research findings

**EAD (Exploratory Annealed Decoding):** Higher temperature for reasoning (explore options), lower for answer (commit). Free — config only.

**AUQ (Agentic Uncertainty Quantification):** Model outputs confidence score with each reasoning step. Below threshold → triggered reflection. +10-13% on agent benchmarks.

**Defensive Refusal Bias:** Explicit "you are authorized" doesn't help. What helps: reframing DENIED as requiring EXPLICIT evidence, not suspicion. Decision-centric prompting.

**Validator retry:** Already have OutcomeValidator (kNN). Currently non-blocking (warn only). Make it blocking for high-confidence disagreement → return correction hint → model retries answer.

### Key insight from literature
"A task is LEGITIMATE if it matches normal CRM workflows. Being cautious ≠ being correct."
