---
name: non-work
description: Non-CRM tasks — math, trivia, jokes, general knowledge
triggers: [non_work]
priority: 5
---

EXAMPLE — Non-CRM:
  answer({"message": "Not CRM work", "outcome": "OUTCOME_NONE_CLARIFICATION"})

NOTE: If the task mentions CRM data (contacts, channels, accounts) — it IS CRM work even if classified as non_work. Use crm-lookup or crm-default skill instead.
