---
name: finance-query
description: Answer practical money questions — bills, invoices, spend, revenue, dates, totals
triggers: [intent_query]
priority: 12
keywords: [total, spend, revenue, how much, sum, balance, cost breakdown]
---

<!-- AI-NOTE: pac1-prod "Finance ops" category — answer money questions from CRM data -->
WORKFLOW:
  1. Identify what financial data is needed (invoices, bills, amounts, dates, totals)
  2. Search relevant directories: my-invoices/, accounts/, or any finance-related folders
  3. Read matching files to extract amounts, dates, line items
  4. Calculate totals, sums, counts as needed — show your math
  5. Answer with precise numbers and file references

RULES:
  - ALWAYS include file references in answer (accounts/acct_XXX.json, my-invoices/INV-XXX.json)
  - For counting: use search() which returns [N matching lines] — report the count
  - For totals: read individual files, extract amounts, sum them explicitly
  - For date ranges: check each file's date field against the requested range
  - When asked "how many" — search first, then count results
  - When asked "how much" — read files, extract amounts, calculate sum
  - Prefer precision over speed — read actual files rather than guessing from filenames

EXAMPLE — Total invoices for an account:
  search({"pattern": "Acme Corp", "path": "my-invoices"}) → 3 matching files
  read({"path": "my-invoices/INV-001-01.json"}) → {"amount": 5000, "date": "2026-01-15"}
  read({"path": "my-invoices/INV-001-02.json"}) → {"amount": 3200, "date": "2026-02-20"}
  read({"path": "my-invoices/INV-001-03.json"}) → {"amount": 7800, "date": "2026-03-10"}
  answer(message="Total: $16,000 across 3 invoices (INV-001-01: $5K, INV-001-02: $3.2K, INV-001-03: $7.8K)", outcome="OUTCOME_OK", refs=["my-invoices/INV-001-01.json", "my-invoices/INV-001-02.json", "my-invoices/INV-001-03.json"])
