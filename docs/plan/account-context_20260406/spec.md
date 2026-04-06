# Specification: Account Context Pre-loading & Paraphrase Resolution

**Track ID:** account-context_20260406
**Type:** Feature
**Created:** 2026-04-06
**Status:** Draft

## Summary

5 tasks (t35, t36, t38, t39, t40) fail because the agent can't reliably resolve account paraphrases like "Benelux compliance-heavy bank account Blue Harbor" or swapped names like "Blom Frederike". Contacts are pre-loaded via `contacts_summary()` but accounts have NO pre-loading — the LLM operates blind on account data, requiring extra search steps that often fail or cause budget exhaustion.

Fix: pre-load accounts into context (parity with contacts), add swapped-name search variant, and annotate account search results.

## Failing Tasks (from benchmark 2026-04-05, Nemotron)

| Task | Score | Hint | Root Cause |
|------|-------|------|------------|
| t35 | 0.00 | security review email from account paraphrase | Can't resolve paraphrase → account name |
| t36 | 0.00 | resend last invoice using account paraphrase | DENIED over-caution (paraphrase not in CRM context) |
| t38 | 0.00 | lookup primary contact email from account paraphrase | Can't resolve "CanalPort" paraphrase |
| t39 | 0.00 | lookup account manager email from account paraphrase | Same as t38 |
| t40 | 0.00 | list accounts for swapped account manager name | "Blom Frederike" ≠ "Frederike Blom" |

## Acceptance Criteria

- [x] `accounts_summary()` in CrmGraph returns name, domain, and linked contacts for all accounts
- [x] Accounts pre-loaded in pregrounding context (parallel to contacts_summary)
- [x] `expand_query()` tries reversed word order for 2-word queries ("Blom Frederike" → also tries "Frederike Blom")
- [x] Account search results annotated with contact info (parity with `annotate_contact_results`)
- [x] `cargo test` passes with new tests for accounts_summary, expand_query swapped, annotate_account_results
- [x] t35, t38, t39, t40 pass on Nemotron (make task T=tXX)
- [ ] No regressions on t01-t34, t37

## Dependencies

- None (all changes internal to agent-bit)

## Out of Scope

- Storing account_manager/description in CRM graph nodes (account files have more fields, but graph only stores name+domain — LLM reads files for details)
- Account paraphrase NLP matching (LLM handles paraphrase resolution from pre-loaded context)
- Changes to sgr-agent

## Technical Notes

- `contacts_summary()` at `crm_graph.rs:447` is the pattern to follow — accounts version mirrors this
- `annotate_contact_results()` at `tools.rs:474` is the pattern for account annotation
- `expand_query()` at `tools.rs:314` currently does "John Smith" → ["John Smith", "Smith", "John"] — needs "Smith John" variant
- Pre-grounding injection point at `pregrounding.rs:424-429` (contacts already injected there)
- Account paraphrase resolution relies on LLM seeing account names in context and matching descriptive terms (e.g., "Blue Harbor" in "Benelux compliance-heavy bank account Blue Harbor" → matches account "Blue Harbor Bank" in summary)
