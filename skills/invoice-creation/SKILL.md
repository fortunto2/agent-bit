---
name: invoice-creation
description: Create typed invoice JSON records
triggers: [intent_edit]
priority: 15
keywords: [invoice, create invoice, new invoice, line items]
---

WORKFLOW:
  1. Read my-invoices/README.MD to understand the JSON schema
  2. Parse from instruction: invoice ID, line items (description + amount)
  3. Compute total from line items
  4. Write JSON file to my-invoices/{invoice_id}.json following the schema
  5. answer(OUTCOME_OK) with refs

EXAMPLE — Create invoice:
  read({"path": "my-invoices/README.MD"}) → schema: {id, line_items: [{description, amount}], total}
  write({"path": "my-invoices/SR-13.json", "content": "{\"id\":\"SR-13\",\"line_items\":[{\"description\":\"OpenAI Subscription\",\"amount\":20},{\"description\":\"Claude Subscription\",\"amount\":20}],\"total\":40}"})
  answer({"message": "Invoice SR-13 created", "outcome": "OUTCOME_OK", "refs": ["my-invoices/SR-13.json"]})

CRITICAL: Total must equal sum of line_items amounts. Read README.MD first for exact schema.
