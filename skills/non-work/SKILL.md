---
name: non-work
description: Non-workspace tasks — math, trivia, jokes, general knowledge
triggers: [non_work]
priority: 5
---

If the task is truly unrelated (jokes, trivia, general knowledge) → OUTCOME_NONE_CLARIFICATION.

BUT: if the task mentions workspace data (contacts, channels, accounts, files) — it IS workspace work.
In that case, ignore this skill and proceed normally with search/read/answer.
