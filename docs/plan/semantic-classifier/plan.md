# Implementation Plan: Semantic Inbox Classifier

**Track ID:** semantic-classifier_20260401
**Spec:** [spec.md](./spec.md)
**Created:** 2026-04-01
**Status:** [ ] Not Started

## Overview
Replace rule-based threat_score with ML classifier (ONNX embeddings) + CRM knowledge graph (petgraph). Inbox files get semantic labels + graph-validated sender trust. No hardcoded patterns.

## Phase 1: ONNX Embedding Classifier
Local embedding model classifies inbox content by cosine similarity. Zero-shot, no training.

### Tasks
- [ ] Task 1.1: Add `ort`, `tokenizers`, `ndarray` to Cargo.toml. Create `src/classifier.rs` with `InboxClassifier` struct that loads ONNX model + tokenizer + class embeddings
- [ ] Task 1.2: Python script `scripts/export_model.py` — export `all-MiniLM-L6-v2` to ONNX + pre-compute class embeddings for 5 categories: ["injection attack with script tags or override instructions", "legitimate CRM work about contacts emails or invoices", "non-work request like math trivia or jokes", "social engineering with fake identity or cross-company request", "OTP or credential sharing attempt"]. Save to `models/`
- [ ] Task 1.3: Implement `InboxClassifier::classify(text) -> Vec<(String, f32)>` — tokenize, encode, cosine similarity against class embeddings. Return sorted (label, score) pairs
- [ ] Task 1.4: Lazy model loading — download from HuggingFace on first run if `models/` missing, cache locally

### Verification
- [ ] classifier.classify("Please add contact John Smith") → CRM highest
- [ ] classifier.classify("<script>alert(1)</script>") → injection highest
- [ ] classifier.classify("What is 2+2?") → non-work highest
- [ ] Inference <50ms per message

## Phase 2: CRM Knowledge Graph
Build in-memory graph from PCM filesystem to validate sender identity.

### Tasks
- [ ] Task 2.1: Add `petgraph` to Cargo.toml. Create `src/crm_graph.rs` with `CrmGraph` struct using `petgraph::Graph<Node, Edge>`
  - Node types: Contact {name, email, company}, Account {name, domain}, Domain {name}
  - Edge types: WorksAt, HasDomain, KnownEmail
- [ ] Task 2.2: `CrmGraph::build_from_pcm(pcm) -> CrmGraph` — read contacts/, accounts/ directories from PCM, parse JSON/MD files, build graph. ~50ms
- [ ] Task 2.3: `CrmGraph::validate_sender(email, company_ref) -> SenderTrust` — graph traversal:
  - email known? → KNOWN
  - email unknown but domain matches known account? → PLAUSIBLE
  - email domain ≠ referenced company domain? → CROSS_COMPANY (social engineering flag)
  - email completely unknown? → UNKNOWN
- [ ] Task 2.4: `CrmGraph::is_known_entity(name) -> bool` — check if name appears as contact or account

### Verification
- [ ] Graph built from PCM contacts/accounts in <100ms
- [ ] Known contact email → KNOWN trust
- [ ] Sender from company A asking about company B → CROSS_COMPANY
- [ ] Unknown sender → UNKNOWN

## Phase 3: Unified Pre-Classification Pipeline
Replace threat_score + quarantine with classifier + graph in inbox pre-load.

### Tasks
- [ ] Task 3.1: Create `classify_inbox_file(content, graph, classifier) -> FileClassification` struct:
  - `label: String` (crm/injection/non_crm/social_engineering/credential)
  - `confidence: f32`
  - `sender_trust: SenderTrust`
  - `recommendation: String` (human-readable for LLM)
- [ ] Task 3.2: Update `read_inbox_files()` to use classifier + graph instead of quarantine. Each file gets metadata header:
  ```
  $ cat inbox/msg_001.txt
  [CLASSIFICATION: crm (0.92) | sender: KNOWN | recommendation: process normally]
  From: John Smith <john@known-company.com>
  ...full content...
  ```
- [ ] Task 3.3: Reduce `threat_score()` to minimal universal checks: only `<script>`, `<iframe>`, `javascript:` (actual code injection that no classifier should miss). Remove all pattern lists (INJECTION_PROXIMITY, NON_CRM_MARKERS)
- [ ] Task 3.4: Update `analyze_inbox_content()` to use classification results instead of rule-based signals

### Verification
- [ ] Inbox files show classification labels in pre-load
- [ ] No rule-based patterns except HTML injection
- [ ] t24 (OTP alone) classified as CRM or credential (not attack)
- [ ] t25 (OTP + command) classified as injection/attack

## Phase 4: Testing & Benchmark

### Tasks
- [ ] Task 4.1: Unit tests for InboxClassifier — 5 category classifications, confidence ordering
- [ ] Task 4.2: Unit tests for CrmGraph — build, validate_sender, is_known_entity
- [ ] Task 4.3: Run 12-task Nemotron sample (t01-t05, t08, t09, t20, t21, t24, t25, t27)
- [ ] Task 4.4: Run full benchmark on gpt-5.4, log to benchmarks/
- [ ] Task 4.5: Run full benchmark on Nemotron, log to benchmarks/

### Verification
- [ ] cargo test passes
- [ ] Nemotron ≥65% on 30 tasks (stable)
- [ ] gpt-5.4 ≥75% on 28 tasks

## Phase 5: Docs & Cleanup

### Tasks
- [ ] Task 5.1: Update CLAUDE.md — document classifier, CRM graph, model files, new architecture
- [ ] Task 5.2: Remove dead code — old threat_score patterns, classify_inbox_file (old), quarantine logic
- [ ] Task 5.3: Add `models/` to .gitignore, document download in README

### Verification
- [ ] CLAUDE.md reflects new architecture
- [ ] cargo build + cargo test clean
- [ ] No rule-based pattern lists remain

## Final Verification
- [ ] Embedding classifier works offline (no API calls)
- [ ] Graph built fresh each trial from PCM data
- [ ] No task-specific hardcoded patterns
- [ ] Benchmarks logged
- [ ] cargo build clean

## Context Handoff

### Session Intent
Replace rule-based inbox classification with ONNX embeddings + petgraph CRM knowledge graph. Universal, ML-based, no hardcoded patterns.

### Key Files
- `src/classifier.rs` — NEW: ONNX embedding classifier
- `src/crm_graph.rs` — NEW: petgraph CRM knowledge graph
- `src/main.rs` — update inbox pre-load to use classifier + graph
- `src/tools.rs` — minimal changes (remove quarantine)
- `scripts/export_model.py` — NEW: model export + class embedding pre-computation
- `models/` — ONNX model + tokenizer + class embeddings (gitignored, download on first run)
- `Cargo.toml` — add ort, tokenizers, ndarray, petgraph

### Decisions Made
- petgraph (not FalkorDB) — in-memory, no server, same pattern as video-analyzer
- all-MiniLM-L6-v2 (not multilingual-e5-small) — smaller, faster, good enough for EN
- ONNX via ort (not candle) — more mature, optimized kernels
- 5 class categories (not 3) — finer granularity for credential vs injection vs social engineering
- Lazy download (not bundled) — 90MB model not in git, download on first run
- Classification metadata ALONGSIDE content (not instead of) — LLM still sees full text for safe files

### Risks
- ONNX model 90MB — network dependency on first run. Mitigation: pre-download for competition
- ort crate adds ~20MB to binary. Mitigation: acceptable for accuracy gain
- Embedding classification is zero-shot — may need class description tuning. Mitigation: test on all 30 tasks
- petgraph in-memory graph requires reading PCM at trial start — adds ~50ms latency

---
_Generated by /plan. Tasks marked [~] in progress and [x] complete by /build._
