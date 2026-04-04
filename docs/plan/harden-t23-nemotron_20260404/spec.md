# Specification: Harden t23 Contact Disambiguation for Nemotron

**Track ID:** harden-t23-nemotron_20260404
**Type:** Bug
**Created:** 2026-04-04
**Status:** Draft

## Summary

t23 "process inbox" passes 75% on GPT-5.4 but **0% on Nemotron** despite contact pre-grounding being implemented (fix-t23-contact-ambiguity_20260403, Phase 1-2 complete). The previous plan's Phase 4 (diagnose + fix for Nemotron) was never executed.

Root cause hypothesis: Nemotron ignores the suggestive "Contact disambiguation hints" user message because (1) it's buried under tree/schema/inbox context, (2) the hint format is informational ("best match: X") rather than directive, and (3) there's no worked example in the prompt showing the disambiguation workflow. Nemotron needs structural, explicit guidance — not subtle hints.

This track diagnoses the exact failure mode, strengthens the disambiguation signals to be Nemotron-compatible, and verifies the fix.

## Acceptance Criteria

- [ ] t23 passes 2/3 runs on Nemotron (currently 0/3)
- [ ] t23 maintains 3/4+ on GPT-5.4
- [ ] No regression on t01 (baseline CRM)
- [ ] No regression on t18 (social engineering)
- [ ] No regression on t19 (legit email resend)
- [ ] Disambiguation hint format is directive, not suggestive
- [ ] Prompt includes explicit disambiguation workflow example

## Dependencies

- Prior track: fix-t23-contact-ambiguity_20260403 (Phase 1-2 complete — CrmGraph, pre-grounding, search annotations)
- `src/pregrounding.rs` — `resolve_contact_hints()` line 100
- `src/prompts.rs` — system prompt, examples
- `src/tools.rs` — SearchTool annotation line 458

## Out of Scope

- Fixing t03/t08 (different root cause: file ops / delete routing)
- Fixing t25/t29 (different root cause: OTP classification)
- NLI model integration
- OutcomeValidator tuning

## Technical Notes

- Evolve log: commit ac784d2 achieved 0.75 on GPT-5.4 with prompt-only fix, 0% Nemotron
- Pre-grounding code in pregrounding.rs:443-457 injects hints as a user message
- Hint format: `- "Smith" → best match: john smith (account: Acme Corp). Others: jane smith`
- System prompt line 12: "Multiple matching contacts? Read both, pick the best match" — insufficient for Nemotron
- Nemotron responds better to: explicit examples, imperative commands, UPPERCASE labels
- Same pattern as t08 fix: prompt hint failed → structural fix succeeded
