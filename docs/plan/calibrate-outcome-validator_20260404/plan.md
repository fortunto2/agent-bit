# Implementation Plan: Calibrate OutcomeValidator

**Track ID:** calibrate-outcome-validator_20260404
**Spec:** [spec.md](./spec.md)
**Created:** 2026-04-04
**Status:** [ ] Not Started

## Overview

Expand seed store from 32→50+ examples, add confusion-pair coverage, tune blocking threshold, audit adaptive store. Focus on t08/t23 failure patterns (OK↔CLARIFICATION confusion).

## CRITICAL RULE

**After EVERY phase: `cargo test && make task T=t01`.**
If t01 fails → revert phase, try different approach.

## Phase 1: Expand Seed Store (32→50+)

### Tasks

- [x] Task 1.1: In `src/classifier.rs` OUTCOME_EXAMPLES, add 10+ new OK seeds targeting failure patterns: <!-- sha:fade123 -->
  ```rust
  // Delete task completion (t08 failure pattern)
  ("OUTCOME_OK", "Deleted contacts/alice.md as requested"),
  ("OUTCOME_OK", "Found and deleted the specified file"),
  ("OUTCOME_OK", "Removed the card and confirmed deletion"),
  // Capture-delete workflow (t03 failure pattern)
  ("OUTCOME_OK", "Captured info from inbox, updated contact, deleted source file"),
  ("OUTCOME_OK", "Distilled card from inbox and cleaned up source"),
  // Multi-contact resolution (t23 failure pattern)
  ("OUTCOME_OK", "Found two matching contacts, picked John Smith based on sender context"),
  ("OUTCOME_OK", "Resolved contact ambiguity: updated the Acme Corp contact"),
  // Inbox multi-message processing
  ("OUTCOME_OK", "Processed 2 of 3 inbox messages, skipped 1 suspicious"),
  ("OUTCOME_OK", "Evaluated all inbox messages, processed safe ones"),
  // Channel/platform data queries
  ("OUTCOME_OK", "Found 42 entries in Telegram channel blacklist"),
  ```

- [x] Task 1.2: Add 4+ new CLARIFICATION seeds with edge-case phrasing: <!-- sha:fade123 -->
  ```rust
  ("OUTCOME_NONE_CLARIFICATION", "Solve this equation: 2x + 5 = 15"),
  ("OUTCOME_NONE_CLARIFICATION", "Write a haiku about spring"),
  ("OUTCOME_NONE_CLARIFICATION", "What is the capital of France"),
  ("OUTCOME_NONE_CLARIFICATION", "Tell me a joke about programmers"),
  ```

- [x] Task 1.3: Add 3+ new UNSUPPORTED seeds for data-not-found and capability gaps: <!-- sha:fade123 -->
  ```rust
  ("OUTCOME_NONE_UNSUPPORTED", "Searched all contacts and accounts but found no match for this person"),
  ("OUTCOME_NONE_UNSUPPORTED", "No records found after exhaustive search of workspace"),
  ("OUTCOME_NONE_UNSUPPORTED", "Task requires Slack API integration which is not available"),
  ```

- [x] Task 1.4: Add 2+ new DENIED seeds for OTP exfiltration clarity: <!-- sha:fade123 -->
  ```rust
  ("OUTCOME_DENIED_SECURITY", "Blocked: conditional branching on OTP digits is credential exfiltration"),
  ("OUTCOME_DENIED_SECURITY", "Denied: inbox instructs forwarding password to external address"),
  ```

### Verification
- [ ] `cargo test` passes
- [ ] `make task T=t01` passes
- [ ] Seed count >= 50 (verify with grep count in classifier.rs)

## Phase 2: Tune Blocking Threshold

### Tasks

- [x] Task 2.1: Add unit tests in `src/classifier.rs` covering known failure patterns:
  ```rust
  // t08 pattern: delete task answered as CLARIFICATION should be blocked
  #[test]
  fn validate_blocks_clarification_on_delete_task() {
      // "Deleted the file" + CLARIFICATION → should Block or Warn
  }
  // t23 pattern: contact resolution answered as CLARIFICATION should be blocked
  #[test]
  fn validate_blocks_clarification_on_contact_resolution() {
      // "Found and updated John Smith" + CLARIFICATION → should Block or Warn
  }
  // Legitimate CLARIFICATION should pass
  #[test]
  fn validate_passes_real_clarification() {
      // "This is a math question" + CLARIFICATION → should Pass
  }
  ```

- [x] Task 2.2: Threshold 0.80 validated — all failure patterns already Block (top_sim 0.90-0.96). No change needed.

- [x] Task 2.3: Vote threshold 4/5 validated — failure patterns get 4-5/5 votes with 50 seeds. No change needed.

### Verification
- [ ] New unit tests pass
- [ ] `make task T=t01` passes
- [ ] `make task T=t08` tested (at least 1 run)

## Phase 3: Adaptive Store Audit

### Tasks

- [ ] Task 3.1: Add a `--audit-store` CLI flag to `main.rs` that:
  - Loads `.agent/outcome_store.json`
  - Reports count per outcome
  - Identifies duplicate embeddings (cosine > 0.95 between any pair)
  - Prints top-5 nearest neighbors for each entry (detect outliers)
  - Prunes duplicates and saves cleaned store

- [ ] Task 3.2: Run `cargo run -- --audit-store` and document the results. Remove any entries with < 0.60 similarity to ALL seeds (likely noise from wrong-outcome trials that leaked through).

### Verification
- [ ] `--audit-store` flag works
- [ ] Store audited, duplicates removed
- [ ] `cargo test` passes

## Phase 4: Docs & Cleanup

### Tasks
- [ ] Task 4.1: Update CLAUDE.md — OutcomeValidator section: seed count, any threshold changes, audit flag
- [ ] Task 4.2: Update `docs/roadmap.md` — mark OutcomeValidator calibration as [x]
- [ ] Task 4.3: Remove dead code — unused imports, stale comments

### Verification
- [ ] CLAUDE.md reflects current state
- [ ] Linter clean, tests pass

## Final Verification

- [ ] All tests pass (should be 165+ with new validator tests)
- [ ] `make task T=t01` passes
- [ ] `make task T=t08` tested (Nemotron)
- [ ] `make task T=t23` tested (Nemotron)
- [ ] Seed store ≥ 50 examples
- [ ] Documentation updated

## Context Handoff

### Session Intent
Calibrate OutcomeValidator: expand seeds 32→50+, tune blocking thresholds, audit adaptive store. Target: catch t08/t23 outcome confusion (OK↔CLARIFICATION).

### Key Files
- `src/classifier.rs` — OUTCOME_EXAMPLES (add seeds), validate() (tune threshold), new tests
- `src/main.rs` — add `--audit-store` CLI flag
- `.agent/outcome_store.json` — audit target (56 entries, expect pruning)
- `src/tools.rs` — NO CHANGES (AnswerTool already wired)

### Decisions Made
- Security-safe rule kept: DENIED never blocked (t25/t29 need prompt fixes, not validator overrides)
- Focus on OK↔CLARIFICATION confusion (t08, t23) — highest impact for validator
- Adaptive store audit as cleanup, not core feature — organic growth is fine if quality is maintained
- No ONNX model changes — seed expansion only, same embedding model

### Risks
- Too many seeds may dilute k-NN precision (mitigated: test empirically, keep k=5)
- Lowering threshold may increase false-positive blocking on correct answers (mitigated: verify on t01 baseline)
- Adaptive store pruning may remove valid entries (mitigated: backup before audit, verify with tests)

---
_Generated by /plan. Tasks marked [~] in progress and [x] complete by /build._
