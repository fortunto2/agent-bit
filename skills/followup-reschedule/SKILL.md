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

EXAMPLE — Reschedule follow-up (placeholders: <ACCOUNT>, <account-id>, <reminder-id>, <OLD-DATE>, <NEW-DATE>):
  search({"pattern": "<ACCOUNT>", "path": "accounts"}) → accounts/<account-id>.json
  read({"path": "accounts/<account-id>.json"}) → {..., "next_follow_up": "<OLD-DATE>", ...}
  write({"path": "accounts/<account-id>.json", "content": "{...same fields, \"next_follow_up\": \"<NEW-DATE>\"}"})
  list({"path": "reminders"}) → reminders/<reminder-id>.json
  read({"path": "reminders/<reminder-id>.json"}) → {..., "date": "<OLD-DATE>", ...}
  write({"path": "reminders/<reminder-id>.json", "content": "{...same fields, \"date\": \"<NEW-DATE>\"}"})
  answer({"message": "Rescheduled follow-up to <NEW-DATE>", "outcome": "OUTCOME_OK", "refs": ["accounts/<account-id>.json", "reminders/<reminder-id>.json"]})

CRITICAL: Update BOTH the account AND the reminder. Missing one = failure.
CRITICAL: Read the FULL JSON before writing — preserve ALL existing fields, only change the date.
