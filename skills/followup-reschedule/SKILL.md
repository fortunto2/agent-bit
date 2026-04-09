---
name: followup-reschedule
description: Reschedule follow-up dates in accounts and reminders
triggers: [intent_edit]
priority: 15
keywords: [reschedule, follow-up, followup, follow up, postpone, move date]
---

WORKFLOW:
  1. Identify the account (by name or description from instruction)
  2. Search accounts/ to find the matching account JSON
  3. Read the account JSON — find next_follow_up or follow_up_date field
  4. Compute the new date:
     - "in two weeks" → current date + 14 days
     - "move to 2026-12-15" → use exact date
  5. Update account JSON with new date (KEEP all other fields unchanged!)
  6. List reminders/ and find the reminder matching the account_id
  7. Update the reminder JSON with the same new date
  8. answer(OUTCOME_OK) with refs to BOTH files

EXAMPLE — Reschedule follow-up:
  search({"pattern": "Acme", "path": "accounts"}) → accounts/acct_003.json
  read({"path": "accounts/acct_003.json"}) → {..., "next_follow_up": "2026-04-01", ...}
  write({"path": "accounts/acct_003.json", "content": "{...same fields, \"next_follow_up\": \"2026-04-15\"}"})
  list({"path": "reminders"}) → reminders/rem_003.json
  read({"path": "reminders/rem_003.json"}) → {..., "date": "2026-04-01", ...}
  write({"path": "reminders/rem_003.json", "content": "{...same fields, \"date\": \"2026-04-15\"}"})
  answer({"message": "Rescheduled follow-up to 2026-04-15", "outcome": "OUTCOME_OK", "refs": ["accounts/acct_003.json", "reminders/rem_003.json"]})

CRITICAL: Update BOTH the account AND the reminder. Missing one = failure.
CRITICAL: Read the FULL JSON before writing — preserve ALL existing fields, only change the date.
