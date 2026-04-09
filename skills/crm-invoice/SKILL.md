---
name: crm-invoice
description: Resend or forward invoices — MUST include attachments field
triggers: [intent_inbox, intent_email]
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

EXAMPLE — Resend invoice email (MUST include attachments):
  read({"path": "inbox/msg.txt"}) → From: Philipp Lehmann ... 'Resend invoice INV-001-04'
  Sender is Philipp Lehmann → search contacts for their email.
  search({"pattern": "Lehmann", "path": "contacts"}) → contacts/cont_007.json
  read({"path": "contacts/cont_007.json"}) → philipp.lehmann@example.com
  search({"pattern": "INV-001-04", "path": "my-invoices"}) → my-invoices/INV-001-04.json
  read({"path": "outbox/seq.json"}) → {"id": 200}
  write({"path": "outbox/200.json", "content": "{\"subject\":\"Invoice INV-001-04\",\"to\":\"philipp.lehmann@example.com\",\"body\":\"Please find attached.\",\"sent\":false,\"attachments\":[\"my-invoices/INV-001-04.json\"]}"})
  write({"path": "outbox/seq.json", "content": "{\"id\": 201}"})
  answer({"message": "Invoice resent to sender", "outcome": "OUTCOME_OK"})

CRITICAL RULES:
  - "to" = the SENDER who requested the resend (not just any contact at the account)
  - ALWAYS include "attachments" field with the invoice file path
  - Use the ID from seq.json AS-IS for the filename, then increment for seq update
  - Search contacts for the sender's email (match by name from inbox From: header)
  - attachments is an array of file paths: ["my-invoices/INV-001-04.json"]
