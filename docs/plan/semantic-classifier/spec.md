# Specification: Semantic Inbox Classifier (ML-based, no rules)

**Track ID:** semantic-classifier_20260401
**Type:** Feature
**Created:** 2026-04-01
**Status:** Draft

## Summary

Replace brittle rule-based `threat_score()` with a universal semantic classifier. Two-tier approach:

1. **Embedding classifier (ONNX, ~25ms)** — `all-MiniLM-L6-v2` runs locally, classifies inbox files by cosine similarity against pre-computed class embeddings. Universal, zero-shot, no patterns to maintain.

2. **FalkorDB knowledge graph** — build a CRM graph from the PCM filesystem at trial start. Known contacts, accounts, email domains → graph nodes. Incoming inbox messages validated against the graph: "is this sender known?", "does this company match the sender's domain?" Catches social engineering without rules.

3. **LLM classification pass** — for borderline cases (embedding confidence 0.4-0.7), use a cheap LLM call with structured output to classify. Same model, one extra call per ambiguous file.

The result: each inbox file gets a classification label + confidence score. The main agent sees pre-classified content, not raw text. No rule-based patterns except literal HTML tags.

## Acceptance Criteria

- [ ] Embedding classifier (`src/classifier.rs`) loads ONNX model, classifies text into 3+ categories with confidence scores
- [ ] FalkorDB graph built from PCM filesystem at trial start (contacts, accounts, domains)
- [ ] Inbox files pre-classified before agent loop: label + confidence per file
- [ ] Agent sees classification metadata alongside content (not instead of)
- [ ] threat_score() reduced to minimal universal checks only (<script>, <iframe>)
- [ ] Nemotron ≥65% on full 30 tasks (stable, not ±4 variance)
- [ ] gpt-5.4 ≥75% on full 28 tasks
- [ ] No hardcoded task-specific patterns

## Dependencies

- `ort` crate (ONNX Runtime for Rust)
- `tokenizers` crate (HuggingFace tokenizers)
- `all-MiniLM-L6-v2` ONNX export (~90MB, download on first run)
- FalkorDB Lite (Rust client or embedded)
- Pre-computed class embeddings (3-5 categories × 384 dims)

## Out of Scope

- Training custom models (use pre-trained + zero-shot)
- Vector search over inbox history (not needed for single-trial evaluation)
- Full RAG pipeline (overkill for PAC1 inbox files)

## Technical Notes

- Solograph uses same embedding model (multilingual-e5-small, 384-dim)
- FalkorDB supports vector indexes natively: `CREATE VECTOR INDEX FOR (n:Contact) ON (n.embedding)`
- Rust FalkorDB client: `falkordb` crate or direct Redis protocol
- ONNX model can be quantized to INT8 (22MB instead of 90MB)
- Class embeddings computed once at build time, bundled as binary
- For competition: model files can be pre-downloaded to avoid network dependency
