# Implementation Plan: Harden t23 Contact Disambiguation for Nemotron

**Track ID:** harden-t23-nemotron_20260404
**Spec:** [spec.md](./spec.md)
**Created:** 2026-04-04
**Status:** [ ] Not Started

## Overview

Diagnose why Nemotron ignores contact disambiguation hints, then strengthen signals through directive hint format + explicit prompt example. Escalation pattern: suggestive hints → directive hints + worked example.

## Phase 1: Diagnose Failure Mode

Run t23 on Nemotron and capture the exact failure pattern to confirm/refute hypothesis.

### Tasks

- [x] Task 1.1: Run `make task T=t23` with full logging. Capture output. Check: (a) are contact hints present in context? (b) does model see `[CONTACT DISAMBIGUATION]` in search results? (c) what outcome does model choose and why? (d) does model attempt to search contacts at all? <!-- sha:eb95c9f (pre-existing diagnosis from spec: hints present but ignored by Nemotron) -->

- [x] Task 1.2: Based on diagnosis, confirm or adjust Phase 2 approach. If hints are absent → fix injection. If hints are present but ignored → strengthen format + add example. If model doesn't search contacts → fix planning prompt. <!-- sha:eb95c9f (confirmed: hints present but ignored → Phase 2 directive format + example) -->

### Verification

- [x] Failure mode documented (model log analyzed)
- [x] Phase 2 approach confirmed

## Phase 2: Strengthen Disambiguation Signals

Make hints impossible for Nemotron to ignore. Three changes: directive hint format, prompt example, planning context.

### Tasks

- [x] Task 2.1: Change `resolve_contact_hints()` in `src/pregrounding.rs:100-147` — replace suggestive format with directive format: <!-- sha:fb1551d -->
  - Old: `- "Smith" → best match: john smith (account: Acme Corp). Others: jane smith`
  - New: `RESOLVED: "Smith" in this inbox = john smith (account: Acme Corp). USE this contact, not: jane smith`
  - Change the injection header from `"Contact disambiguation hints:"` to `"⚠ CONTACT RESOLUTION (use these, do NOT ask for clarification):"` in pregrounding.rs:450-451

- [x] Task 2.2: Add explicit disambiguation example in `src/prompts.rs` `examples_for_class()` default branch <!-- sha:fb1551d -->

- [x] Task 2.3: Update planning prompt in `src/prompts.rs` PLANNING_PROMPT — add contact ambiguity pattern <!-- sha:fb1551d -->

- [x] Task 2.4: Update unit tests for `resolve_contact_hints()` in `src/pregrounding.rs` to match new directive format <!-- sha:fb1551d -->

### Verification

- [x] `cargo test` passes (131/131)
- [x] `cargo build` succeeds

## Phase 3: Verify & Regression

### Tasks

- [ ] Task 3.1: Run `make task T=t23` 3 times on Nemotron. Target: 2/3 pass minimum.
- [ ] Task 3.2: Regression: `make task T=t01` + `make task T=t18` + `make task T=t19` on Nemotron. All must pass.
- [ ] Task 3.3: If t23 fails 3/3 on Nemotron: escalate to structural fix — consider injecting resolved contact name directly into the instruction text (rewrite instruction with resolved name before LLM sees it). Create follow-up task if needed.

### Verification

- [ ] t23 passes 2/3 on Nemotron
- [ ] No regression on t01, t18, t19

## Phase 4: Docs & Cleanup

### Tasks

- [ ] Task 4.1: Update CLAUDE.md — note directive hint format and disambiguation example in Key Design Decisions
- [ ] Task 4.2: Update `docs/roadmap.md` — mark t23 status based on results

### Verification

- [ ] CLAUDE.md reflects current project state
- [ ] Linter clean, tests pass

## Final Verification

- [ ] All acceptance criteria from spec met
- [ ] Tests pass
- [ ] Linter clean
- [ ] Build succeeds
- [ ] Documentation up to date

## Context Handoff

_Summary for /build to load at session start — keeps context compact._

### Session Intent

Make t23 contact disambiguation work on Nemotron (0% → 2/3+) by strengthening hint format and adding worked examples.

### Key Files

- `src/pregrounding.rs` — `resolve_contact_hints()` lines 100-147, injection at lines 443-457
- `src/prompts.rs` — system prompt line 12, PLANNING_PROMPT line 43, `examples_for_class()` default branch line 106
- `src/tools.rs` — `annotate_contact_results()` lines 458-484

### Decisions Made

- **Directive over suggestive:** Nemotron ignores informational hints. Use imperative "RESOLVED: X = Y. USE this contact." Same pattern that worked for t08 (prompt → structural escalation).
- **Example-driven:** Add a worked disambiguation example — Nemotron follows examples better than instructions.
- **Preserve CrmGraph code:** Phase 1-2 from prior track is correct. Only changing the hint FORMAT and prompt EXAMPLES, not the graph logic.
- **Escalation path:** If directive hints + example still fail, Phase 3 Task 3.3 has a structural fallback (rewrite instruction with resolved names).

### Risks

- Directive hints could be too aggressive — might force wrong contact when disambiguation ranking is wrong. Mitigated: only triggers on genuinely ambiguous matches (2+ contacts).
- Adding more examples increases prompt length. Mitigated: the example is short (5 lines) and only injected for the default class (most tasks).
- Nemotron variance means even a correct fix may show 2/3 instead of 3/3. Accept 2/3 as passing.

---
_Generated by /plan. Tasks marked [~] in progress and [x] complete by /build._
