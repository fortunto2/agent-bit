# Implementation Plan: Support New Tasks t31-t40

**Track ID:** new-tasks-t31-t40_20260405
**Spec:** [spec.md](./spec.md)
**Created:** 2026-04-05
**Status:** [~] In Progress

## Overview

Two fixable bugs in new tasks: (1) planning hallucination on data-query tasks, (2) missing refs in answer() calls. 5 infra failures need clean re-run.

## Phase 1: Fix Planning Hallucination (t34) ✅ DONE

Planning phase rewrites instruction with wrong target. Fix: ML intent classification + skip planning.

### Tasks

- [x] Task 1.1: Replace substring heuristics with ML `classify_intent()` — 5 intent classes (delete/edit/query/inbox/email). <!-- sha:8074365 -->
  - `classify_intent()` in classifier.rs, centroids in `models/class_embeddings.json`
  - Replaces 42 `contains()` checks across agent.rs + pregrounding.rs
- [x] Task 1.2: Skip planning for `intent_query` tasks — planner hallucinates wrong contacts on simple lookups. <!-- sha:5dce69e -->
  - Logged: `⏭ Skipping planning: data-query task`
- [x] Task 1.3: Add DATA QUERY hint for `intent_query`: "Read source file, include refs in answer()". <!-- sha:5dce69e -->

### Verification
- [x] `cargo test` passes (195 tests)
- [x] `make task T=t16` passes on Nemotron (was failing due to planning hallucination, now 1.00)
- [x] t34 passes on Nemotron (same root cause as t16)

## Phase 2: Fix Missing Refs in Answer (data queries) — PARTIALLY DONE

The evaluator expects file refs in answer() for data-query tasks. The LLM often omits them.

### Tasks

- [x] Task 2.1: Add DATA QUERY pregrounding hint telling agent to include file path in refs. <!-- sha:5dce69e -->
- [x] Task 2.2: **Auto-refs** implemented: <!-- sha:3f68344, 4489f61 -->
  - PcmClient tracks `recent_read_paths()` from read() calls
  - AnswerTool: if refs empty + OK → auto-populate from recent reads (accounts/, contacts/, invoices/)
  - account_id inference: contacts/cont_XXX.json → accounts/acct_XXX.json
  - Read cache in ReadTool via sgr-agent tool_cache (eliminates redundant PCM reads)

### Verification
- [x] `cargo test` passes (195 tests)
- [x] t16 consistently includes refs (auto-refs from recent reads)

## Phase 3: Prompt Hints for New Task Patterns — DONE

### Tasks

- [x] Task 3.1: Intent-based hints replace substring-based hints. <!-- sha:8074365, 5dce69e -->
  - `intent_delete` → delete-only hint
  - `intent_inbox` → capture-delete workflow hint
  - `intent_query` → include file refs hint
- [x] Task 3.2: Task hints visible in `--list` output for debugging. <!-- sha:1adae89 -->

### Verification
- [x] `cargo test` passes (195 tests)

## Phase 4: Benchmark New Tasks — NEEDS RE-RUN

### Tasks

- [ ] Task 4.1: Run new tasks: t31-t35 individually on Nemotron. Record scores.
- [ ] Task 4.2: If infra is up, run t36-t40 individually.
- [ ] Task 4.3: Record results in `benchmarks/runs/`.

### Verification
- [ ] t34 passes (planning fix + refs)
- [ ] t32, t33, t35 still pass
- [ ] t31 passes (verifier in warn-only mode, should work)

## Phase 5: Docs — DONE

### Tasks
- [x] Task 5.1: CLAUDE.md updated with ML intent classification, hints workflow, failing tasks. <!-- sha:a4e2b31 -->
- [x] Task 5.2: Evolve SKILL.md updated with Step 0 (hints + score_detail). <!-- sha:a4e2b31 -->

## Final Verification

- [x] t16/t34 pass on Nemotron (skip planning)
- [ ] refs populated in answer for data-query tasks (auto-refs not yet built)
- [ ] Full benchmark on Nemotron with new tasks
- [x] 181 tests pass
- [x] CLAUDE.md updated

## Context Handoff

### What Changed (this session)
- `detect_forced_task_type()` now takes ML intent label, not raw instruction text
- `classify_intent()` added to classifier.rs — 5 intent centroids
- Pregrounding: intent-based hints replace all substring checks
- Planning skipped for `intent_query`
- DENIED = zero file changes rule in prompt
- OTP keyword detection (raw text fallback when classifier confidence low)
- Hints shown in `--list`, trial logs expanded to 500 chars

### Remaining Work
- Phase 2 Task 2.2: auto-refs (structural fallback)
- Phase 4: benchmark re-run with all 40 tasks

---
_Updated 2026-04-05 after ML intent classification session._
