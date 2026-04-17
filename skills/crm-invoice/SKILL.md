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

EXAMPLE — Resend invoice email (placeholders: <SENDER>, <LAST>, <INV-ID>, <N> — not real values):
  read({"path": "inbox/<msg-file>"}) → From: <SENDER> ... 'Resend invoice <INV-ID>'
  Sender is <SENDER> → search contacts for their email.
  search({"pattern": "<LAST>", "path": "contacts"}) → contacts/<contact-file>.json
  read({"path": "contacts/<contact-file>.json"}) → <sender-email>
  search({"pattern": "<INV-ID>", "path": "my-invoices"}) → my-invoices/<INV-ID>.json
  read({"path": "outbox/seq.json"}) → {"id": <N>}
  write({"path": "outbox/<N>.json", "content": "{\"id\":<N>,\"subject\":\"Invoice <INV-ID>\",\"to\":\"<sender-email>\",\"body\":\"Please find attached.\",\"sent\":false,\"attachments\":[\"my-invoices/<INV-ID>.json\"]}"})
  write({"path": "outbox/seq.json", "content": "{\"id\": <N+1>}"})
  answer({"message": "Invoice resent to sender", "outcome": "OUTCOME_OK"})

CRITICAL RULES:
  - "to" = the SENDER who requested the resend (not just any contact at the account)
  - ALWAYS include "attachments" field with the invoice file path
  - Use the ID from seq.json AS-IS for the filename, then increment for seq update
  - Search contacts for the sender's email (match by name from inbox From: header)
  - `attachments` is an array of file paths, e.g. `["my-invoices/<INV-ID>.json"]`
