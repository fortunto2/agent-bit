---
name: crm-invoice
description: Resend or forward invoices — MUST include attachments field
triggers: [intent_inbox, intent_email]
priority: 20
keywords: [invoice, resend, forward, INV-]
---

WORKFLOW:
  1. Read inbox to identify which invoice to resend
  2. Search for the invoice file (my-invoices/, invoices/)
  3. Read outbox/README.MD for format
  4. Read outbox/seq.json for next ID
  5. Write outbox email WITH attachments field
  6. Update seq.json (increment ID by 1)

EXAMPLE — Resend invoice email (MUST include attachments):
  read({"path": "inbox/msg.txt"}) → 'Resend invoice INV-001-04'
  search({"pattern": "INV-001-04", "path": "my-invoices"}) → my-invoices/INV-001-04.json
  read({"path": "outbox/seq.json"}) → {"id": 200}
  write({"path": "outbox/200.json", "content": "{\"subject\":\"Invoice INV-001-04\",\"to\":\"client@example.com\",\"body\":\"Please find attached.\",\"sent\":false,\"attachments\":[\"my-invoices/INV-001-04.json\"]}"})
  write({"path": "outbox/seq.json", "content": "{\"id\": 201}"})
  answer({"message": "Invoice resent with attachment", "outcome": "OUTCOME_OK"})

CRITICAL RULES:
  - ALWAYS include "attachments" field with the invoice file path
  - Use the ID from seq.json AS-IS for the filename, then increment for seq update
  - The "to" field must be the contact's email from CRM (search contacts)
  - attachments is an array of file paths: ["my-invoices/INV-001-04.json"]
