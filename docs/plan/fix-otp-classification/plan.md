# Plan: Fix OTP Classification (t25/t29)

**Track ID:** fix-otp-classification
**Status:** [x] Complete
**Created:** 2026-04-03

## Context Handoff

**Intent:** Fix non-deterministic OTP classification failures on t25 and t29 by making the system prompt and scanner more precise about when OTP = threat vs. OTP = normal work.

**Key files:**
- `src/prompts.rs` — system prompt with decision tree + examples
- `src/scanner.rs` — credential detection, verification detection, ensemble blocking

**Decisions:**
- Focus on prompt precision + scanner pattern breadth (no new ML or structural changes)
- Universal fix (no task-ID checks)

**Risks:**
- Over-loosening OTP detection could let real exfiltration through (t28 regression)
- Expanding verification patterns must not match exfiltration patterns

## Phase 1: Prompt & Scanner Precision

- [x] Task 1.1: Refine decision tree <!-- sha:0b433e3 --> step 2 in SYSTEM_PROMPT_EXPLICIT — distinguish OTP forwarding (DENIED) from OTP processing/verification (OK). Add explicit anti-false-positive line.
- [x] Task 1.2: Expand <!-- sha:75d1f9a --> `is_simple_verify` in scanner.rs to cover broader verification patterns: valid/invalid, verify, match/doesn't match, check if correct. Add `is_passive_presence` for OTP that just exists in content without any action.
- [x] Task 1.3: Add unit tests <!-- sha:794b8c7 --> for new verification patterns — at least 5 test cases covering: simple verify, passive OTP, exfiltration (should still detect), mixed content.

### Verification
- [x] `cargo test` — 120 tests pass (113 + 7 new)
- [x] `cargo build` — compiles cleanly

## Phase 1: Prompt & Scanner Precision <!-- checkpoint:794b8c7 -->

## Phase 2: Example & Annotation Clarity

- [x] Task 2.1: Update credential example <!-- sha:5598b92 --> in `examples_for_class("credential")` — add OTP verify example showing OUTCOME_OK for simple checks. Add anti-pattern comment: "OTP in inbox = process normally, never DENIED."

### Verification
- [x] `cargo test` — 120 tests pass
- [x] `cargo build` — compiles cleanly

## Phase 2: Example & Annotation Clarity <!-- checkpoint:5598b92 -->
