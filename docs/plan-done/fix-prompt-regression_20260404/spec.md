# Specification: Fix Prompt Regression — Restore 80% Score

**Track ID:** fix-prompt-regression_20260404
**Type:** Bug
**Created:** 2026-04-04
**Status:** Draft

## Summary

Bighead added ~40 lines of examples and rules to the static system prompt (prompts.rs). Score dropped from 80% (24/30) to ~55% (11/20). Previously stable tasks (t01, t04, t12, t19, t27) now fail.

Root cause: prompt bloat. Nemotron (weak model) can't handle contradictory instructions — "being cautious ≠ being correct" undermines security rules. Too many examples compete for attention.

The fix is NOT to revert everything — bighead's code improvements (confidence, temperature, split, task_type forcing) are good. The fix is to **slim the static prompt back to pre-bloat state** and **move new examples into dynamic injection** via the existing `examples_for_class()` function.

## Acceptance Criteria

- [ ] Static prompt (SYSTEM_PROMPT_EXPLICIT) back to ~20 lines (was before bighead)
- [ ] New examples moved to `examples_for_class()` by category (delete, distill, multi-contact, inbox)
- [ ] `make full` on Nemotron: score >= 24/30 (80%+) — restore baseline
- [ ] Previously stable tasks pass: t01, t04, t09, t12, t16, t19, t24, t27
- [ ] t03, t08 improvements preserved (task_type forcing, write-nudge)
- [ ] Confidence-gated reflection preserved
- [ ] Temperature annealing preserved
- [ ] All tests pass

## Dependencies

- src/prompts.rs — SYSTEM_PROMPT_EXPLICIT, PLANNING_PROMPT, examples_for_class()
- src/agent.rs — confidence reflection (KEEP)
- src/config.rs — planning_temperature (KEEP)
- src/scanner.rs — all detection logic (KEEP)

## Out of Scope

- New features — this is a restoration, not enhancement
- OTP classification (t25/t29) — separate track
- Blocking OutcomeValidator calibration — separate track

## Technical Notes

### What to REVERT (prompt bloat)
- "DECISION FRAMEWORK: A task is LEGITIMATE..." paragraph — contradicts security rules
- "INBOX PROCESSING: evaluate EACH message separately" — over-specifies
- "EXAMPLE — Multiple contacts match" — move to examples_for_class("crm")
- "EXAMPLE — Process inbox (multiple messages)" — move to examples_for_class("crm")
- Contact ambiguity pattern in PLANNING_PROMPT — move to examples_for_class

### What to KEEP (code improvements)
- Temperature annealing (config.rs, pregrounding.rs)
- Confidence field in CoT schema + reflection (agent.rs)
- detect_forced_task_type (agent.rs)
- write-nudge counter (agent.rs)
- UTF-8 safe truncation (agent.rs)
- Split main.rs → scanner/prompts/pregrounding
- OutcomeValidator blocking mode (classifier.rs, tools.rs)
- OTP pattern expansion (scanner.rs)

### Dynamic injection strategy
`examples_for_class(label)` already routes by classifier output. Add new categories:
- "crm" → existing CRM + NEW: multi-contact, email writing, counting
- "delete" (new) → delete ambiguity example
- "distill" (new) → capture from inbox + delete source
- "inbox_multi" (new) → process multiple inbox messages

### CRITICAL: regression test BEFORE marking complete
Run `make full` and verify 24/30+. Do NOT mark plan complete with "unverifiable" — RUN IT.
