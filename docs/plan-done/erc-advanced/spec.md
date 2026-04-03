# Specification: ERC Advanced — Smart Search + Answer Validation

**Track ID:** erc-advanced_20260331
**Type:** Feature
**Created:** 2026-03-31
**Status:** Draft

## Summary

Apply advanced ERC R2 patterns to fix the specific failure categories from gpt-5.4 benchmark (64%, 9 failures):

- **5 missed traps** (t04/t08/t18/t20/t22): model answers OK when content is non-CRM or injected. Fix: answer validation self-check before submitting.
- **2 over-cautious** (t23/t24): model flags legit tasks as unsupported/clarification. Fix: answer validation catches false positives.
- **1 wrong severity** (t25): CLARIFICATION instead of DENIED. Fix: severity calibration in validator.
- **CRM search quality**: model searches "John Smith" and misses partial matches. Fix: smart search with auto-retry using name parts, fuzzy regex.

PCM search is server-side ripgrep — cannot modify. Smart search wraps it: generates query variants client-side, merges results. Answer validation is a lightweight self-check in `answer` tool before submitting.

## Acceptance Criteria

- [ ] SearchTool auto-retries with partial name queries (surname, first name) when initial search returns 0 results
- [ ] SearchTool generates fuzzy regex variants for names (e.g. "Sm.th" for typo tolerance)
- [ ] AnswerTool validates outcome before submitting — self-check prompt in tool description
- [ ] Answer validation catches trap tasks (non-CRM content → should not be OK)
- [ ] Answer validation catches over-cautious responses (legit CRM → should not be UNSUPPORTED)
- [ ] gpt-5.4 ≥72% on full 26 tasks (from 64%)
- [ ] No regression on Nemotron trap detection (t09, t21 stay 1.00)
- [ ] cargo test passes

## Dependencies

- Existing Pac1Agent (erc-patterns track, complete)
- PCM Search API (server-side, ripgrep)

## Out of Scope

- Vector embeddings / FAISS (no local embedding model)
- Multi-agent delegation (too complex for current scope)
- LLM reranking (extra LLM call per step, too expensive)
- Self-consistency / majority vote (3x cost)

## Technical Notes

- PCM search accepts regex patterns → fuzzy = regex approximation ("Sm[i]?th" instead of exact "Smith")
- Query expansion happens in SearchTool::execute, not in LLM — deterministic, zero cost
- Answer validation is in AnswerTool description (prompt engineering) + prescan of answer content
- Name splitting: "John Smith" → try ["John Smith", "Smith", "John"] as separate queries
- ERC R2 insight: Teams 2, 6, 16 all used query expansion; Team 22 used answer validation
