# Plan: Fix t08 — Delete Ambiguity Resolution

**Track ID:** fix-t08-delete-ambiguity_20260403
**Spec:** [spec.md](spec.md)
**Status:** [ ] Not Started

## Context Handoff

**Intent:** t08 fails on Nemotron because ambiguous delete references ("that card", "the file") aren't resolved before deletion. GPT-5.4 handles this naturally; Nemotron needs explicit prompt guidance.

**Key files:** `src/prompts.rs` (system prompt, planning prompt, examples), `src/agent.rs` (reasoning tool verification)

**Approach:** Prompt-only changes (per roadmap: prompt > classifier > structural > new code). Four targeted edits across 2 files.

**Risks:** Over-constraining delete behavior could regress capture/distill tasks (t03). Keep new guidance narrowly scoped to "ambiguous reference" cases, not all deletes.

## Phase 1: Prompt Guidance

- [~] Task 1.1: Add delete disambiguation rule to system prompt (step 8 area) — `src/prompts.rs:32`
- [ ] Task 1.2: Add delete-with-disambiguation pattern to planning prompt — `src/prompts.rs:60`
- [ ] Task 1.3: Add delete disambiguation example to default CRM examples — `src/prompts.rs:139`
- [ ] Task 1.4: Add delete safety cue to reasoning tool verification description — `src/agent.rs:180`

### Verification
- [ ] `cargo test` passes (120+ tests)
- [ ] `cargo build` succeeds with no warnings
