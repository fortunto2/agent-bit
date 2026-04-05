# Specification: NLI Zero-Shot Classifier

**Track ID:** nli-zero-shot_20260405
**Type:** Feature
**Created:** 2026-04-05
**Status:** Draft

## Summary

Add a cross-encoder NLI (Natural Language Inference) model as a third signal in the inbox classification ensemble. The current classifier uses a bi-encoder (all-MiniLM-L6-v2) with cosine similarity to class centroids — fast but shallow. Cross-encoder NLI models process (premise, hypothesis) pairs jointly, understanding logical relationships rather than just semantic proximity.

**Target tasks: t25, t29 only** (OTP trust distinction). NLI does NOT help t03 (execution workflow), t08 (truncation detection), t18 (domain lookalike — already fixed with strsim), or t23 (trust annotation).

The approach reuses the existing `ort` + `tokenizers` infrastructure (no new Rust deps). A small cross-encoder model (~22-66M params) is exported to ONNX alongside the existing embedding model.

## Acceptance Criteria

- [ ] NLI model exported to ONNX in `models/nli_model.onnx` via `scripts/export_nli_model.py`
- [ ] `NliClassifier` struct in `classifier.rs` — takes (premise, hypothesis) → entailment probability
- [ ] `zero_shot_classify()` method: runs NLI against all class hypotheses, returns sorted scores
- [ ] NLI integrated as third signal in `semantic_classify_inbox_file()` ensemble
- [ ] Ensemble weights tuned: ML + NLI + structural (sum to 1.0)
- [ ] Unit tests for NLI classifier (model-gated, skip if models/ missing)
- [ ] `cargo test` passes (all 178+ existing tests green)
- [ ] t25 and t29 pass rate improves on Nemotron (currently ~50% and ~40%)
- [ ] No regression on t01 and other stable tasks

## Dependencies

- `ort` 2.0.0-rc.12 (already in Cargo.toml)
- `tokenizers` 0.21 (already in Cargo.toml)
- Cross-encoder NLI model: `cross-encoder/nli-deberta-v3-xsmall` (22M params) or `cross-encoder/nli-MiniLM2-L6-H768` (66M params)
- Python dependencies for export: `transformers`, `onnxruntime`, `onnx`, `torch`

## Out of Scope

- Replacing the existing bi-encoder (it stays as a fast first-pass signal)
- NLI for outcome validation (OutcomeValidator) — separate future track
- rust-bert dependency (too heavy; ONNX-based approach is lighter and reuses existing infra)
- Training or fine-tuning NLI models

## Technical Notes

- **Cross-encoder vs bi-encoder:** Bi-encoder encodes text and class separately (fast, O(n) per class). Cross-encoder processes (text, class_hypothesis) together (slower, O(n) per class, but captures interaction). For 5 classes, 5 NLI forward passes per classification.
- **NLI output format:** logits [batch, 3] → [contradiction, neutral, entailment]. We use softmax(logits)[entailment_idx] as the score. Note: label order varies by model — must check `model.config.label2id`.
- **Ensemble integration:** Current is `0.7*ML + 0.3*structural`. With NLI: `0.5*ML + 0.3*NLI + 0.2*structural` (or similar — tune on failing tasks).
- **Model size:** deberta-v3-xsmall ~90MB ONNX, MiniLM2 ~260MB ONNX. Both fit in models/ (gitignored).
- **Performance:** NLI adds ~5 forward passes per inbox file classification. Each pass ~10-50ms on CPU. Acceptable for PAC1 trial timing.
- **Prompt diet lesson applies:** Nemotron needs redundancy. NLI adds a safety net for the LLM's weaker reasoning — belt + suspenders + NLI harness.
