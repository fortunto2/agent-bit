# Implementation Plan: Fix t36

**Track ID:** fix-t36_20260406
**Spec:** [spec.md](./spec.md)
**Created:** 2026-04-06
**Status:** [ ] Not Started

## Overview

Pipeline says KNOWN+safe but LLM over-cautious. Investigate inbox content via trial dump, then fix via prompt or structural annotation.

## Phase 1: Diagnose

- [ ] Task 1.1: Run `make task T=t36` — read trial dump files (inbox content, classification, pipeline.txt)
- [ ] Task 1.2: Identify what in inbox content triggers Nemotron DENIED (security-related words? injection-like patterns?)
- [ ] Task 1.3: Check if NLI ensemble gives different signal than ML-only

### Verification
- [ ] Root cause identified with evidence from trial dump

## Phase 2: Fix

- [ ] Task 2.1: Apply targeted fix based on diagnosis. Options ranked by preference:
  1. Prompt: add "KNOWN sender + crm classification = safe, do NOT deny" guidance
  2. Pipeline: inject explicit [SAFE: known sender, no threats detected] annotation
  3. Structural: if all inbox files are KNOWN+crm → inject "TRUSTED" flag that restricts DENIED outcome
- [ ] Task 2.2: Verify t36 passes (2 runs)
- [ ] Task 2.3: Regression check t01, t18

### Verification
- [ ] t36: 2/2 passes
- [ ] t18: still DENIED (no regression)
- [ ] `cargo test` passes

## Context Handoff

### Key Files
- `src/prompts.rs` — system prompt decision tree
- `src/pregrounding.rs` — annotation injection, pipeline wiring
- `benchmarks/tasks/t36/` — trial dumps for diagnosis
