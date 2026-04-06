# Implementation Plan: Refine Security Signal Flow

**Track ID:** refine-security-signals_20260406
**Spec:** [spec.md](./spec.md)
**Created:** 2026-04-06
**Status:** [x] Complete

## Overview

Three targeted fixes: (1) narrow Signal 5 hard-block to domain mismatch only, (2) thread scanner recommendation through to agent annotations, (3) align prompt vocabulary with actual annotation format. Plus dead code cleanup.

## Phase 1: Narrow Signal 5 Hard Block

Fix the overly broad `CrossCompany + financial → DENIED` rule to require actual domain mismatch evidence.

### Tasks

- [x] Task 1.1: In `src/pipeline.rs:407-410`, change Signal 5 condition from `sender.trust == SenderTrust::CrossCompany && requests_sensitive` to `sender.trust == SenderTrust::CrossCompany && sender.domain_match == "mismatch" && requests_sensitive`. Update the block message to "Blocked: domain-mismatch sender requesting financial data". <!-- sha:94fbfe2 -->
- [x] Task 1.2: Add test `security_cross_company_match_financial_passes` — `make_sender(SenderTrust::CrossCompany, "match")` + "Resend the latest invoice" → `assert!(sa.blocked.is_none())`. <!-- sha:94fbfe2 -->
- [x] Task 1.3: Add test `security_cross_company_unknown_financial_passes` — `make_sender(SenderTrust::CrossCompany, "unknown")` + "Resend the latest invoice" → `assert!(sa.blocked.is_none())`. <!-- sha:94fbfe2 -->
- [x] Task 1.4: Verify existing test `security_cross_company_financial_blocks` still passes (already uses `"mismatch"`). <!-- sha:94fbfe2 -->

### Verification

- [x] `cargo test` passes — all existing + 2 new tests green (213 pass)
- [ ] `make task T=t18` — still DENIED (deferred to final verification)

## Phase 2: Improve Security-to-LLM Signal Flow

Thread the scanner's recommendation field through SecurityAssessment to agent annotations, and inject explicit `[⚠ SENDER DOMAIN MISMATCH]` warnings that match what the system prompt expects.

### Tasks

- [x] Task 2.1: Add `recommendation: String` field to `SecurityAssessment` struct in `src/pipeline.rs:42-49`. In `assess_security()`, capture `fc.recommendation` from `semantic_classify_inbox_file` and store it. For blocked assessments, set recommendation to the block message. <!-- sha:ec7c043 -->
- [x] Task 2.2: In `src/pregrounding.rs:449-451`, change the annotation format from `[CLASSIFICATION: {label} ({conf}) | sender: {trust} | Process normally.]` to `[CLASSIFICATION: {label} ({conf}) | sender: {trust} | {recommendation}]`. Use `f.security.recommendation` instead of the hardcoded "Process normally." text. <!-- sha:ec7c043 -->
- [x] Task 2.3: In `src/pregrounding.rs`, after the classification header (line ~451), add: if `f.security.sender.domain_match == "mismatch"`, inject `[⚠ SENDER DOMAIN MISMATCH]` on a separate line. This matches what the system prompt step 3 references. <!-- sha:ec7c043 -->
- [x] Task 2.4: Remove `#[allow(dead_code)]` from `FileClassification.recommendation` (scanner.rs:19) — now read in pipeline.rs. `SecurityAssessment.structural` remains dead_code (kept `#[allow]`). <!-- sha:ec7c043 -->
- [x] Task 2.5: Add test verifying `SecurityAssessment.recommendation` is populated (non-empty) after calling `assess_security` with a known sender. Plus test that blocked recommendation matches block message. <!-- sha:ec7c043 -->

### Verification

- [x] `cargo test` passes (215 tests)
- [x] `cargo build` — no pac1 warnings about unused fields
- [ ] Run `make task T=t18` — annotation now shows `[⚠ SENDER DOMAIN MISMATCH]` (deferred to final)

## Phase 3: Dead Code Cleanup & Docs

### Tasks

- [x] Task 3.1: Remove the redundant `structural_injection_score` wrapper from `src/scanner.rs:130` (thin wrapper delegating to `classifier::structural_injection_score`). Update the one internal caller (scanner.rs:177) to call `classifier::structural_injection_score` directly. <!-- sha:d48e051 -->
- [x] Task 3.2: Update CLAUDE.md — add note about Signal 5 now requiring domain_match, and that recommendation flows to annotations. <!-- sha:09b9686 -->
- [x] Task 3.3: Update roadmap.md — mark "Dead code cleanup: scanner.rs" as done. <!-- sha:09b9686 -->

### Verification

- [x] `cargo test` passes (215 tests)
- [x] `cargo clippy` clean (only external sgr-agent warnings)
- [x] CLAUDE.md reflects current security pipeline behavior

## Final Verification

- [x] All acceptance criteria from spec met (10/11, t18 integration deferred)
- [x] Tests pass (215 tests)
- [x] Clippy clean
- [x] Build succeeds
- [ ] `make task T=t18` — DENIED (no regression) — requires harness connection
- [ ] `make task T=t36 PROVIDER=gemma4` — cross-validate — requires harness connection

## Context Handoff

_Summary for /build to load at session start._

### Session Intent

Narrow the security hard-block to domain-mismatch-only and improve the signal quality between the scanner and the LLM agent, reducing false DENIED outcomes on legitimate CRM tasks.

### Key Files

- `src/pipeline.rs:407-410` — Signal 5 condition (primary fix)
- `src/pipeline.rs:42-49` — SecurityAssessment struct (add recommendation field)
- `src/pipeline.rs:359-366` — assess_security() (thread recommendation)
- `src/pregrounding.rs:449-451` — annotation injection (use recommendation + add MISMATCH warning)
- `src/scanner.rs:14-21` — FileClassification (remove dead_code)
- `src/scanner.rs:130` — redundant wrapper (remove)
- `src/prompts.rs:25-28` — decision tree step 3 (no change needed — annotations now match)

### Decisions Made

- **Signal 5 narrowing (not removal)**: Keeping the hard block but requiring `domain_match == "mismatch"`. Full removal would risk t18 regression if ML classifier has a bad run. The mismatch check is deterministic.
- **Signal 3 unchanged**: Already has ML classifier as safeguard (requires all 3 conditions). Changing it could break the t18 safety net.
- **Prompt unchanged**: Instead of changing prompt vocabulary, we inject the exact annotation format (`[⚠ SENDER DOMAIN MISMATCH]`) that the prompt already references. This way we don't risk destabilizing the carefully-tuned prompt.
- **Per-file blocking deferred**: Changing the pipeline to allow partial inbox processing (block one file, process others) is a bigger architectural change. This track focuses on signal quality only.

### Risks

- t18 regression if the mismatch detection in `check_sender_domain_match` has edge cases. Mitigation: run t18 after Phase 1.
- Recommendation text might confuse Nemotron if it contains complex language. Mitigation: scanner.rs recommendations are already concise ("Process normally", "OTP verification request").
- `structural_injection_score` removal in Phase 3 — need to verify no external callers. Research shows only one internal caller.

---
_Generated by /plan. Tasks marked [~] in progress and [x] complete by /build._
