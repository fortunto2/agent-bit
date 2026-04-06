# Specification: Refine Security Signal Flow

**Track ID:** refine-security-signals_20260406
**Type:** Bug
**Created:** 2026-04-06
**Status:** Draft

## Summary

The pipeline's security assessment (Signal 5 in `assess_security`) hard-blocks ALL `SenderTrust::CrossCompany` senders requesting financial data — even when the domain actually matches. This causes false DENIED outcomes on legitimate cross-company CRM tasks (t36, and contributes to non-deterministic failures on t02, t18 bad runs).

Additionally, `scanner.rs` computes a helpful `recommendation` field (e.g., "Process normally — no secret is leaked") that is marked `#[allow(dead_code)]` and **never reaches the agent**. The LLM only sees cryptic `sender: CROSS_COMPANY` without context, while the system prompt references `[⚠ SENDER DOMAIN MISMATCH]` — a format that doesn't exist in actual annotations. This vocabulary mismatch causes Nemotron to guess incorrectly on edge cases.

## Acceptance Criteria

- [x] Signal 5 only hard-blocks when `domain_match == "mismatch"` (not all CrossCompany)
- [x] New test: CrossCompany + domain match + financial → NOT blocked
- [x] New test: CrossCompany + domain unknown + financial → NOT blocked
- [x] Existing test: CrossCompany + domain mismatch + financial → still blocked
- [x] `SecurityAssessment` carries `recommendation` from scanner classification
- [x] Agent annotations include recommendation text (e.g., `[CLASSIFICATION: crm (0.85) | sender: KNOWN | OTP verification — process normally.]`)
- [x] `[⚠ SENDER DOMAIN MISMATCH]` annotation injected only when `domain_match == "mismatch"`, matching what the system prompt references
- [x] Dead `#[allow(dead_code)]` annotations removed from fields that are now read
- [x] Redundant `structural_injection_score` wrapper removed from scanner.rs
- [x] `cargo test` passes (215 tests)
- [ ] No regressions on t18 (lookalike — must still be DENIED) — deferred to integration run

## Dependencies

- None (internal refactor of existing security pipeline)

## Out of Scope

- Changing Signal 3 logic (ML threat + sender_suspect + sensitive — already has ML classifier safeguard)
- Per-file blocking (blocking only the suspicious file, not entire inbox) — separate track
- Prompt content changes beyond annotation vocabulary alignment
- New ML model training or NLI hypothesis tuning

## Technical Notes

- Signal 5 (pipeline.rs:407-410) was added for t18 (invoice from lookalike). The intent was correct but implementation is too broad: it checks `trust == CrossCompany` without checking `domain_match`.
- `validate_sender()` in crm_graph.rs returns CrossCompany in two distinct cases: (1) sender domain doesn't match referenced company's domain, (2) sender domain stem resembles a known account (lookalike). Only case 2 with `domain_match == "mismatch"` should hard-block.
- The `FileClassification.recommendation` field is computed with nuanced logic in scanner.rs:230-282 (credential verification vs exfiltration, confidence-based messages) — valuable signal being discarded.
- The system prompt step 3 expects `[⚠ SENDER DOMAIN MISMATCH]` annotation but pregrounding.rs only injects `sender: CROSS_COMPANY` — Nemotron can't reliably map between these.
