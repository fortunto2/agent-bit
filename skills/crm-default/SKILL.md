---
name: crm-default
description: General workspace tasks — email writing, contact updates, file operations
triggers: [crm, intent_edit, intent_email, intent_inbox]
priority: 1
---

CRITICAL: These are WORKFLOW PATTERNS, not scripts. Use REAL data from the workspace, never copy example text.
Your answer message must describe what YOU actually did with REAL file names and data.
EFFICIENCY: Minimize tool calls. Use read_all() for small dirs. Use search() — it auto-reads when ≤10 matches.

WORKFLOW — Email writing:
  1. read outbox/README.MD — get required JSON fields
  2. read outbox/seq.json — get next ID
  3. write outbox/{id}.json — email with all required fields + "sent": false
  4. write outbox/seq.json — increment ID by 1
  CRITICAL: Always update seq.json after writing email. Missing = task failure.

WORKFLOW — Cross-account detection:
  Before processing inbox requests: resolve sender → contact → account.
  If sender's account ≠ requested data's account → OUTCOME_NONE_CLARIFICATION.

WORKFLOW — Multiple contacts match:
  Search → read ALL matches → pick best match by context. NEVER give up with CLARIFICATION.

WORKFLOW — Inbox (multiple messages):
  Evaluate EACH message separately. Process safe ones, skip dangerous. OK if ≥1 processed.

WORKFLOW — Update file:
  Read file → write IMMEDIATELY with changes. Do NOT re-read.

WORKFLOW — External API / deploy / calendar:
  Cannot do → OUTCOME_NONE_UNSUPPORTED (not DENIED, not OK).
