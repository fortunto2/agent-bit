# Specification: Fix t23 Over-Cautious DENIED on Contact Ambiguity

**Track ID:** fix-t23-contact-ambiguity_20260403
**Type:** Bug
**Created:** 2026-04-03
**Status:** Draft

## Summary

t23 "process inbox" fails non-deterministically (0% Nemotron, 75% GPT-5.4) because the agent encounters multiple matching contacts during execution and either gives up with CLARIFICATION or panics into DENIED. Root cause: contact ambiguity is discovered late (inside the LLM execution loop) with no disambiguation hints. The prompt guidance at line 79 ("Multiple matching contacts? Read both, pick the best match") helps GPT-5.4 but is insufficient for weaker models.

The fix: pre-resolve contact ambiguity during pre-grounding (before the LLM loop starts) using the CRM graph. Extract names mentioned in inbox emails, search contacts, and inject disambiguation hints into the context. Additionally, annotate search results with CRM graph context when multiple contacts match.

## Acceptance Criteria

- [ ] t23 passes 3/3 runs on Nemotron (currently 0/3) — needs harness verification
- [ ] t23 passes 3/3 runs on GPT-5.4 (currently 3/4) — needs harness verification
- [ ] No regression on t18 (social engineering detection) — needs harness verification
- [ ] No regression on t19 (legit email resend — fixed last track) — needs harness verification
- [ ] No regression on t01, t09, t16, t24 (clean CRM tasks) — needs harness verification
- [x] Contact pre-grounding extracts names from inbox and resolves via CRM graph
- [x] Search results in contacts/ annotated with best-match hint when multiple files match

## Dependencies

- CRM graph: `src/crm_graph.rs` — contact/account/domain resolution
- Pre-grounding: `src/main.rs` lines 1245-1316 — context injection
- Smart search: `src/tools.rs` lines 337-393 — query expansion + auto-expand
- Inbox reader: `src/main.rs` lines 1003-1081 — classified inbox content

## Out of Scope

- NLI model integration (Architecture TODO)
- OutcomeValidator blocking mode
- Fixing t03/t08 (execution failures — different root cause)
- Fixing t25/t29 (OTP handling — different root cause)

## Technical Notes

- Prior evolve attempt (ac784d2) added prompt hint → 75% GPT-5.4, 0% Nemotron. Prompt-only fix insufficient.
- CRM graph already built at line 1271 (`CrmGraph::build_from_pcm`), available for pre-grounding.
- Contact names can be extracted from inbox emails via From: header display name or body mentions.
- `CrmGraph.fuzzy_find_contact()` exists (Levenshtein >0.7) but is only used for sender validation, not pre-grounding.
- `check_sender_domain_match` already correlates sender domain with accounts — same logic can correlate contacts.
- The `auto_expand_search` in tools.rs already expands <=3 file results — could add CRM hints there.
