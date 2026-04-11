---
name: crm-lookup
description: Data queries — find contacts, lookup emails, count entries, search channels
triggers: [intent_query]
priority: 10
---

WORKFLOW:
  1. Search for the target (contacts/, accounts/, docs/channels/)
  2. Read the found file to extract the answer
  3. Include file path in refs when calling answer()

EXAMPLE — Data lookup:
  search({"pattern": "Smith", "path": "contacts"}) → contacts/john-smith.md:3:John Smith
  read({"path": "contacts/john-smith.md"}) → John Smith <john@acme.com>
  answer({"message": "Found contact John Smith", "outcome": "OUTCOME_OK", "refs": ["contacts/john-smith.md"]})

EXAMPLE — Counting (how many X):
  search({"pattern": "blacklist", "path": "docs/channels/Telegram.txt"}) → [788 matching lines]
  The number in brackets [N matching lines] IS the answer. Do NOT recount manually.
  answer({"message": "788", "outcome": "OUTCOME_OK"})
  IMPORTANT: Use the EXACT keyword from the question (e.g. "blacklist" not " - blacklist").
  The search count = the answer. Trust search results, do NOT re-search or re-count.

EXAMPLE — Find captured article (by date or keyword):
  list({"path": "01_capture/influential"}) → [files with dates in filenames]
  Calculate date from 'X days ago' → find matching filename.
  FOUND → read it, answer with refs:
    read({"path": "01_capture/influential/2026-03-06__article.md"}) → [content]
    answer({"message": "Found: article title", "outcome": "OUTCOME_OK", "refs": ["01_capture/influential/2026-03-06__article.md"]})
  NOT FOUND (no file for that date) → CLARIFICATION (not OK, not UNSUPPORTED):
    answer({"message": "No article captured on that date", "outcome": "OUTCOME_NONE_CLARIFICATION"})

IMPORTANT: Always include refs in your answer. For counting, use search — it returns [N matching lines].
