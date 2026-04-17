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

OUTLINE — Reschedule follow-up:
  search(<account name or keyword>, accounts path) → <account file>
  read(<account file>) → note the current `next_follow_up` / `follow_up_date` value
  write(<account file>, full original JSON with ONLY the date field updated to the new value)
  list(reminders path) → locate the reminder whose `account_id` matches the account
  read + write that reminder file the same way, keeping every other field intact
  answer(OUTCOME_OK, refs = [account_file, reminder_file])
  Use paths and dates from THIS trial — do not reuse placeholders.

CRITICAL: Update BOTH the account AND the reminder. Missing one = failure.
CRITICAL: Read the FULL JSON before writing — preserve ALL existing fields, only change the date.
