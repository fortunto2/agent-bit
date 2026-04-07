# Implementation Plan: Stabilize Nemotron

**Track ID:** stabilize-nemotron_20260408
**Spec:** [spec.md](./spec.md)
**Created:** 2026-04-08
**Status:** [ ] Not Started

## Approach

For each failing task: run with DUMP_TRIAL, fetch BitGN log, compare passing vs failing runs.
Use workflow state machine hooks where possible. Block > Warn (Nemotron ignores warnings).

## Phase 1: t21 — DENIED vs CLARIFICATION

- [ ] Task 1.1: Run t21 3x, fetch BitGN logs for failures
- [ ] Task 1.2: Root cause: is ML classifier label wrong? Is V2 prompt unclear?
- [ ] Task 1.3: Try: add non_work example to V2 prompt, or workflow guard for non-CRM content
- [ ] Regression: t01, t05

## Phase 2: t19 — model over-caution

- [ ] Task 2.1: Run t19 3x on Nemotron, compare with GPT-5.4 BitGN log
- [ ] Task 2.2: Compare: what does GPT-5.4 Verify step say vs Nemotron?
- [ ] Task 2.3: Try: stronger [✓ TRUSTED] annotation, or workflow guard for KNOWN sender
- [ ] Regression: t18 (lookalike must still DENIED)

## Phase 3: t29 — OTP oracle

- [ ] Task 3.1: Run t29 5x, categorize trial seeds (trusted/untrusted handle × OTP match/mismatch)
- [ ] Task 3.2: Fix if pattern emerges, otherwise document as non-deterministic

## Phase 4: t23 — multi-inbox step budget

- [ ] Task 4.1: t23 on Nemotron already ~0% — try GPT-5.4 v2 (confirmed 1.00)
- [ ] Task 4.2: On Nemotron: try limiting read count via workflow Block (>10 reads → Block read, force write)
- [ ] Regression: t01, t03

## Context Handoff

### Key Files
- `src/workflow.rs` — WorkflowState, advance_step, pre_action, post_action
- `src/hooks.rs` — HookRegistry (from_agents_md)
- `src/prompts.rs` — V2 prompt, examples_for_class
- `src/policy.rs` — file protection (scan_content, check_write)

### Key finding from this session
- Block > Warn for Nemotron (model ignores warnings)
- GPT-5.4 vs Nemotron comparison reveals exact failure point
- BitGN log at trial.harness_url/?format=json&offset=0 = ground truth
