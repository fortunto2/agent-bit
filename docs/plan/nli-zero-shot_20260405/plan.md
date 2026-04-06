# Implementation Plan: NLI Zero-Shot Classifier

**Track ID:** nli-zero-shot_20260405
**Spec:** [spec.md](./spec.md)
**Created:** 2026-04-05
**Status:** [ ] Not Started

## Overview

Add a cross-encoder NLI model (ONNX) as a third signal in the inbox classification ensemble. Reuses existing `ort` + `tokenizers` infra — no new Rust dependencies. Export script produces `models/nli_model.onnx` + `models/nli_tokenizer.json`. `NliClassifier` in `classifier.rs` performs zero-shot classification via entailment scoring. Integrated into `scanner.rs` ensemble with tuned weights.

## Phase 1: NLI Model Export

Export a cross-encoder NLI model to ONNX format.

### Tasks

- [x] Task 1.1: Create `scripts/export_nli_model.py` <!-- sha:0880676 --> — export `cross-encoder/nli-deberta-v3-xsmall` to ONNX. Inputs: (input_ids, attention_mask, token_type_ids). Output: logits [batch, 3]. Record `label2id` mapping (entailment index) in `models/nli_config.json`. Include verification step comparing ONNX output to PyTorch.
- [x] Task 1.2: Run export script, verify model files exist: `models/nli_model.onnx` (273MB), `models/nli_tokenizer.json` (8MB), `models/nli_config.json`. Correlation 1.0. <!-- sha:0880676 -->

### Verification

- [x] `models/nli_model.onnx` exists and is loadable by `onnxruntime`
- [x] Export script prints verification: ONNX vs PyTorch entailment scores match (>0.95 correlation)

## Phase 2: NliClassifier Implementation

Rust struct for NLI inference using `ort` + `tokenizers`.

### Tasks

- [x] Task 2.1: Add `NliClassifier` struct — load/is_available/try_load + nli_config.json entailment_idx <!-- sha:a861c94 -->
- [x] Task 2.2: Implement `entailment_score()` — sentence pair tokenization + softmax + token_type_ids <!-- sha:a861c94 -->
- [x] Task 2.3: Implement `zero_shot_classify()` — sorted entailment scores for all hypotheses <!-- sha:a861c94 -->
- [x] Task 2.4: Define `NLI_HYPOTHESES` v2 — tuned for CRM (0.778) + credential (0.636) discrimination <!-- sha:a861c94 -->
- [x] Task 2.5: 7 model-gated unit tests — CRM top, exfil→credential/injection top2, score range, inject≠crm <!-- sha:a861c94 -->

### Verification

- [x] `cargo test` passes (202 tests — 195 existing + 7 NLI)
- [x] NLI correctly distinguishes CRM from credential (0.778 vs 0.069 entailment)

## Phase 3: Ensemble Integration

Blend NLI scores into `scanner.rs` inbox classification.

### Tasks

- [x] Task 3.1: Add `SharedNliClassifier` type alias in `scanner.rs`, create in `main.rs` <!-- sha:d3e12b4 -->
- [x] Task 3.2: Thread `SharedNliClassifier`: main → run_trial → run_agent → scan_inbox → assess_security → semantic_classify_inbox_file <!-- sha:d3e12b4 -->
- [x] Task 3.3: 3-way ensemble: 0.5*ML + 0.3*NLI + 0.2*structural, NLI override when >0.5 confidence <!-- sha:d3e12b4 -->
- [x] Task 3.4: Updated all test calls with NLI parameter (pipeline + scanner tests) <!-- sha:d3e12b4 -->

### Verification

- [x] `cargo test` passes (202 tests green)
- [x] `cargo build` clean (no new warnings in agent-bit)

## Phase 4: Benchmark & Tune

Verify no regression, tune ensemble weights on failing tasks.

### Tasks

- [x] Task 4.1: No regression — t01=1.0, t02=1.0, t05=1.0, t09=1.0, t16=1.0 on Nemotron <!-- sha:d3e12b4 -->
- [x] Task 4.2: t25/t29 unchanged — NLI scores <0.04 on structured OTP messages (expected). NLI adds signal on natural language text only (CRM=0.778, credential=0.636 on long text). <!-- sha:d3e12b4 -->
  - [x] No regression: ensemble graceful degradation works (falls through to ML when NLI is low)
  - [x] NLI impact: additive on natural text, neutral on structured messages

### Verification

- [x] No regression on stable tasks (t01, t02, t05, t09, t16 = 1.0)
- [x] NLI classification annotations visible in trial logs (`[NLI] scores: ...`)

## Phase 5: Docs & Cleanup

### Tasks

- [~] Task 5.1: Update CLAUDE.md — add NLI classifier to Architecture section, document ensemble weights, add `export_nli_model.py` usage
- [ ] Task 5.2: Update roadmap.md — mark `NLI model for zero-shot classification` as `[x]` done, note ONNX approach instead of rust-bert
- [ ] Task 5.3: Remove dead code — unused imports, stale comments from integration

### Verification

- [ ] CLAUDE.md reflects current project state
- [ ] `cargo test` passes, `cargo build` clean

## Final Verification

- [ ] All acceptance criteria from spec met
- [ ] Tests pass (177+ existing + new NLI tests)
- [ ] Build clean (no warnings)
- [ ] `make sample` >= 80% on Nemotron
- [ ] Documentation up to date

## Context Handoff

_Summary for /build to load at session start — keeps context compact._

### Session Intent

Add cross-encoder NLI model as third signal in inbox classification ensemble to improve non-deterministic task stability.

### Prerequisite Context (2026-04-05 session)

- `classifier.rs` now has TWO classify methods: `classify()` (security labels) and `classify_intent()` (intent labels). NLI should integrate with `classify()` only (security labels).
- `classify_filtered()` is the internal method — NLI can plug in at the same level.
- `scripts/export_model.py` now has `SECURITY_CLASSES` and `INTENT_CLASSES` dicts — NLI hypotheses should align with `SECURITY_CLASSES` labels.
- Test count is 181 (not 177).
- `scanner.rs::semantic_classify_inbox_file()` is the ensemble entry point (ML + structural + sender trust).

### Key Files

- `scripts/export_nli_model.py` — NEW: export cross-encoder NLI to ONNX
- `src/classifier.rs` — ADD: `NliClassifier` struct, `entailment_score()`, `zero_shot_classify()`, `NLI_HYPOTHESES`
- `src/scanner.rs` — MODIFY: `semantic_classify_inbox_file()` to accept NLI and compute 3-way ensemble
- `src/main.rs` — MODIFY: create `SharedNliClassifier`, thread through to pregrounding
- `src/pregrounding.rs` — MODIFY: pass NLI classifier to scanner calls
- `models/nli_model.onnx` — NEW: exported NLI model (gitignored)
- `models/nli_config.json` — NEW: label2id mapping

### Decisions Made

- **ONNX over rust-bert:** Reuses existing `ort` + `tokenizers` infrastructure. No new heavy deps (rust-bert pulls libtorch ~2GB). Same pattern as existing `InboxClassifier`.
- **cross-encoder/nli-deberta-v3-xsmall (22M params):** Smallest viable NLI model. ~90MB ONNX. Fast enough for per-file classification (5 hypotheses × ~10-50ms = 50-250ms total).
- **3-way ensemble (0.5/0.3/0.2):** NLI replaces some ML weight, not structural. Structural signals (NFKC, zero-width, base64) are deterministic and should keep weight. Weights are tunable in Phase 4.
- **Graceful degradation:** If NLI model not present, falls back to existing 2-way ensemble. No hard dependency.

### Risks

- **NLI model size:** ~90MB adds to gitignored models/. Acceptable (existing model.onnx.data is already 90MB).
- **Latency:** 5 NLI passes per file × N inbox files. For typical PAC1 trials (1-3 inbox files), ~250-750ms total. Within timing budget.
- **Label order:** Cross-encoder models have varying `label2id` mappings. Must read from config, not hardcode.
- **Tokenizer difference:** NLI model may use different tokenizer than existing MiniLM. Separate `nli_tokenizer.json` file.

---
_Generated by /plan. Tasks marked [~] in progress and [x] complete by /build._
