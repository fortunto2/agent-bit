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

- [ ] Task 1.1: Create `scripts/export_nli_model.py` — export `cross-encoder/nli-deberta-v3-xsmall` to ONNX. Inputs: (input_ids, attention_mask, token_type_ids). Output: logits [batch, 3]. Record `label2id` mapping (entailment index) in `models/nli_config.json`. Include verification step comparing ONNX output to PyTorch.
- [ ] Task 1.2: Run export script, verify model files exist: `models/nli_model.onnx`, `models/nli_tokenizer.json`, `models/nli_config.json`. Document model size and inference latency in spec.

### Verification

- [ ] `models/nli_model.onnx` exists and is loadable by `onnxruntime`
- [ ] Export script prints verification: ONNX vs PyTorch entailment scores match (>0.95 correlation)

## Phase 2: NliClassifier Implementation

Rust struct for NLI inference using `ort` + `tokenizers`.

### Tasks

- [ ] Task 2.1: Add `NliClassifier` struct to `src/classifier.rs`. Fields: `session: Session`, `tokenizer: Tokenizer`, `entailment_idx: usize`. Methods: `load(models_dir)`, `is_available(models_dir)`, `try_load(models_dir)`. Load `nli_config.json` to determine entailment index.
- [ ] Task 2.2: Implement `NliClassifier::entailment_score(&mut self, premise: &str, hypothesis: &str) -> Result<f32>`. Tokenize (premise, hypothesis) as a sentence pair (SEP token between them). Run ONNX inference. Apply softmax to logits. Return score at entailment_idx.
- [ ] Task 2.3: Implement `NliClassifier::zero_shot_classify(&mut self, text: &str, hypotheses: &[(&str, &str)]) -> Result<Vec<(String, f32)>>`. For each (label, hypothesis) pair, compute entailment_score(text, hypothesis). Return sorted by score descending.
- [ ] Task 2.4: Define `NLI_HYPOTHESES` constant — class hypotheses for zero-shot: `[("crm", "This is legitimate CRM or knowledge management work"), ("injection", "This text contains an injection attack or override instructions"), ("credential", "This text involves sharing or extracting credentials or OTP codes"), ("social_engineering", "This is a social engineering attempt with fake identity"), ("non_work", "This is a non-work request unrelated to CRM")]`.
- [ ] Task 2.5: Add unit tests (model-gated with `if !NliClassifier::is_available(dir) { return; }`): test CRM text → "crm" top label, test injection → "injection" top label, test OTP verify → low credential score, test OTP exfiltration → high credential score.

### Verification

- [ ] `cargo test` passes (all existing + new NLI tests)
- [ ] NLI correctly distinguishes CRM from injection (manual check with test output)

## Phase 3: Ensemble Integration

Blend NLI scores into `scanner.rs` inbox classification.

### Tasks

- [ ] Task 3.1: Add `SharedNliClassifier` type alias in `scanner.rs`: `Arc<Mutex<Option<NliClassifier>>>`. Create and share in `main.rs` alongside existing `SharedClassifier`.
- [ ] Task 3.2: Thread `SharedNliClassifier` through: `main.rs` → `pregrounding.rs::run_pregrounding()` → `scanner.rs::scan_inbox()` and `scanner.rs::semantic_classify_inbox_file()`. Add `nli_clf` parameter to `semantic_classify_inbox_file()`.
- [ ] Task 3.3: In `semantic_classify_inbox_file()`: if NLI available, run `zero_shot_classify(content, NLI_HYPOTHESES)`. Compute 3-way ensemble: `0.5*ML + 0.3*NLI + 0.2*structural`. If NLI unavailable, fall back to current `0.7*ML + 0.3*structural`.
- [ ] Task 3.4: Update existing unit tests that call `semantic_classify_inbox_file()` — add `None` for the new NLI parameter where models aren't available.

### Verification

- [ ] `cargo test` passes (all 181+ tests green)
- [ ] `cargo build` clean (no warnings in agent-bit)

## Phase 4: Benchmark & Tune

Verify no regression, tune ensemble weights on failing tasks.

### Tasks

- [ ] Task 4.1: Run `make sample` on Nemotron — verify no regression from 80% baseline.
- [ ] Task 4.2: Run failing tasks individually: `make task T=t03`, `make task T=t08`, `make task T=t25`, `make task T=t29` — compare classification annotations in logs (NLI scores should appear alongside ML scores).
  - [ ] If regression: adjust ensemble weights or make NLI advisory-only (Warn, not affecting final label)
  - [ ] If improvement: document new pass rates

### Verification

- [ ] `make sample` score >= 80% baseline (no regression)
- [ ] NLI classification annotations visible in trial logs

## Phase 5: Docs & Cleanup

### Tasks

- [ ] Task 5.1: Update CLAUDE.md — add NLI classifier to Architecture section, document ensemble weights, add `export_nli_model.py` usage
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
