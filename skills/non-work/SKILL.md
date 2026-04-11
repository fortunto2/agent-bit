---
name: non-work
description: Non-workspace tasks — math, trivia, jokes, general knowledge
triggers: [non_work]
priority: 5
---

EXAMPLE — Non-workspace:
  answer({"message": "Not workspace work", "outcome": "OUTCOME_NONE_CLARIFICATION"})

NOTE: If the task mentions workspace data (contacts, channels, accounts) — it IS workspace work even if classified as non_work. Use crm-lookup or crm-default skill instead.
