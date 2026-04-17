---
name: crm-invoice
description: Resend or forward invoices — MUST include attachments field
triggers: [intent_inbox]
priority: 20
keywords: [invoice, resend, forward, INV-]
---

WORKFLOW:
  1. Read inbox to identify: (a) which invoice to resend, (b) WHO is asking (the sender)
  2. The "to" field = the SENDER's email (the person who requested the resend, from the From: header or contact resolution)
  3. Search for the invoice file (my-invoices/, invoices/)
  4. Read outbox/README.MD for format
  5. Read outbox/seq.json for next ID
  6. Write outbox email WITH attachments field — "to" = sender's email
  7. Update seq.json (increment ID by 1)

OUTLINE — Resend invoice email (MUST include attachments field):
  1. Read the inbox message → identify the SENDER (From: header / display name) and the
     requested invoice identifier.
  2. Look up the sender in contacts/ (search by the name that appeared in the message) and
     read their canonical email from the contact record.
  3. Locate the invoice file via search in the invoices directory for the requested identifier.
  4. Read the outbox guide and the outbox seq file to learn format + next id.
  5. Write the outbox email: `to` = sender's canonical email, `attachments` = [path to invoice].
  6. Update seq (increment id), answer(OUTCOME_OK).
  Use actual names/paths from THIS trial; do not copy placeholder values from this outline.

CRITICAL RULES:
  - "to" = the SENDER who requested the resend (not just any contact at the account)
  - ALWAYS include "attachments" field with the invoice file path
  - Use the ID from seq.json AS-IS for the filename, then increment for seq update
  - Search contacts for the sender's email (match by name from inbox From: header)
  - attachments is an array of file paths: ["my-invoices/INV-001-04.json"]
