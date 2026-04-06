# Implementation Plan: Switch V2 Default

**Track ID:** switch-v2-default_20260407
**Spec:** [spec.md](./spec.md)
**Created:** 2026-04-07
**Status:** [ ] Not Started

## Phase 1: Validate

- [ ] Task 1.1: Wait for v2 benchmark results (running now)
- [ ] Task 1.2: Compare v2 vs explicit scores per task
- [ ] Task 1.3: If v2 ≥ 75%: proceed. If < 75%: investigate regressions.

## Phase 2: Switch

- [ ] Task 2.1: Set `prompt_mode = "v2"` in nemotron provider (not nemotron-v2 separate)
- [ ] Task 2.2: Run 8-task sample to confirm
- [ ] Task 2.3: Update CLAUDE.md — document V2 prompt, when to use explicit vs v2

## Context Handoff

### Key Files
- `config.toml` — provider prompt_mode
- `src/prompts.rs` — SYSTEM_PROMPT_V2 vs SYSTEM_PROMPT_EXPLICIT
- `src/pregrounding.rs` — template selection logic
