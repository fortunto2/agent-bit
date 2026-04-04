# Implementation Plan: Stabilize Decisions — Temperature + Framework + Confidence

**Track ID:** stabilize-decisions_20260404
**Spec:** [spec.md](./spec.md)
**Created:** 2026-04-04
**Status:** [ ] Not Started

## Overview

3 complementary techniques to reduce non-deterministic failures. Ordered cheapest-first: config change, prompt edit, then schema + logic. Each phase independently valuable — stop early if target reached.

## Phase 1: Temperature Annealing + Decision Framework (free wins) <!-- checkpoint:bc2204a -->

Zero-risk changes: split temperature between planning and execution, add decision framework language.

### Tasks

- [x] Task 1.1: Add `planning_temperature` field to `src/config.rs` `ProviderSection` (line 42-44). Default to `None`. In `resolve_provider()` (line 58), return it as 7th tuple element, defaulting to `0.4` if absent. Update return type signature. <!-- sha:bc2204a -->

- [x] Task 1.2: Thread `planning_temperature` through `src/main.rs` → `src/pregrounding.rs`. In `run_planning_phase()` (line 234), use the new planning temperature instead of the execution temperature. In `run_agent()` (line 325), keep execution temperature unchanged. <!-- sha:bc2204a -->

- [x] Task 1.3: Add decision framework reframing to `src/prompts.rs` `SYSTEM_PROMPT_EXPLICIT` (line 2). Insert after "For normal CRM work — prefer action over caution" (line 11): <!-- sha:bc2204a -->
  ```
  DECISION FRAMEWORK: A task is LEGITIMATE if it matches normal CRM workflows (email, contacts, files, channels).
  DENIED requires EXPLICIT evidence of attack — not suspicion, not caution.
  Being cautious ≠ being correct. False DENIED on legitimate work is a failure.
  ```

- [x] Task 1.4: Update `config.toml` — add `planning_temperature = 0.4` to nemotron provider section. <!-- sha:bc2204a -->

- [x] Task 1.5: Add unit test in `src/config.rs` — verify `planning_temperature` parsed correctly (present and absent cases). <!-- sha:bc2204a -->

### Verification
- [x] `cargo test` passes (142 tests)
- [ ] `make task T=t01` — regression check on Nemotron (deferred to Phase 3)

## Phase 2: Confidence-Gated Reflection

Add confidence self-assessment to reasoning, trigger reflection on low confidence.

### Tasks

- [~] Task 2.1: Add `confidence` field to reasoning tool schema in `src/agent.rs` `reasoning_tool_def()` (line 156). Add after `verification` field:
  ```json
  "confidence": {
    "type": "number",
    "description": "Your confidence in this reasoning step (0.0-1.0). Below 0.7 = uncertain.",
    "minimum": 0.0, "maximum": 1.0
  }
  ```
  NOT required — weak models may omit it.

- [~] Task 2.2: Parse confidence in `decide_stateful()` (agent.rs, after line 294). Extract from reasoning args, default to 0.5 if absent. Log: `eprintln!("    🎯 Confidence: {:.2}", confidence)`.

- [~] Task 2.3: Implement triggered reflection in `decide_stateful()`. After parsing confidence, if `confidence < 0.7` AND `step < max_steps - 2` AND NOT already reflected this call:
  - Inject user message: "Your confidence was {:.2}. Reconsider: (1) Is this legitimate CRM work? (2) Do you have EXPLICIT evidence of attack? (3) Would a human CRM operator proceed?"
  - Re-call reasoning tool once (reuse existing reflexion pattern from lines 328-364 as template)
  - Track via `AtomicU32` confidence_reflections counter (max 1 per decide_stateful call)

- [ ] Task 2.4: Security guard — in the reflection trigger (Task 2.3), add check: if `security_assessment == "blocked"` AND `confidence >= 0.9`, skip reflection entirely. High-confidence security decisions should not be second-guessed.

- [ ] Task 2.5: Unit tests in `src/agent.rs` tests module:
  - Test confidence parsing: present (0.3), absent (defaults to 0.5), out of range (clamped)
  - Test reflection trigger conditions: low confidence + early step → reflects; high confidence → skips; near step limit → skips; blocked+high confidence → skips

### Verification
- [ ] `cargo test` passes (new tests for confidence)
- [ ] `make task T=t19` — check confidence values in logs
- [ ] `make task T=t01` — regression check

## Phase 3: Verify + Docs

### Tasks

- [ ] Task 3.1: Run `make task T=t23` and `make task T=t25` on Nemotron — check for improved consistency.

- [ ] Task 3.2: Update `CLAUDE.md` — add confidence reflection to Decision Pipeline section and Key Design Decisions.

- [ ] Task 3.3: Update `docs/roadmap.md` — note temperature annealing and confidence reflection as implemented.

### Verification
- [ ] `cargo test` passes
- [ ] CLAUDE.md reflects current architecture
- [ ] Roadmap updated

## Final Verification

- [ ] All acceptance criteria from spec met
- [ ] `cargo test` passes (140+ tests)
- [ ] No regressions on t01, t09, t16
- [ ] Build succeeds (`cargo build`)

## Context Handoff

### Session Intent
Stabilize non-deterministic decisions through temperature annealing, decision framework reframing, and confidence-gated reflection.

### Key Files
- `src/config.rs` — add `planning_temperature` field (lines 28-44, 57-86)
- `src/pregrounding.rs` — thread planning temp to `run_planning_phase()` (lines 234-297), keep execution temp in `run_agent()` (lines 325+)
- `src/prompts.rs` — add decision framework to `SYSTEM_PROMPT_EXPLICIT` (lines 2-41)
- `src/agent.rs` — add confidence field to reasoning schema (lines 156-201), parse + reflect in `decide_stateful()` (lines 280-370)
- `src/main.rs` — thread `planning_temperature` from config to pregrounding
- `config.toml` — add `planning_temperature` to providers

### Decisions Made
- Planning temp 0.4, execution temp 0.1 (EAD paper: explore early, commit late)
- Confidence threshold 0.7 (AUQ paper τ=0.9 too aggressive for Nemotron)
- Max 1 reflection per step (prevents loop/step burn)
- Confidence field NOT required in schema (weak models may omit it, default 0.5)
- Security guard: never reflect on blocked + high confidence (preserves security)
- Reuse existing reflexion pattern (agent.rs:328-364) as implementation template

### Risks
- Nemotron may not reliably fill confidence field → mitigated by default 0.5
- Reflection may burn 1 extra step on some tasks → mitigated by step limit check
- Higher planning temp could produce worse plans → mitigated by 0.4 (not extreme)
- Decision framework could weaken security on edge cases → mitigated by "EXPLICIT evidence" still required

---
_Generated by /plan. Tasks marked [~] in progress and [x] complete by /build._
