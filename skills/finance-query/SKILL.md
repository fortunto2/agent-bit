---
name: finance-query
description: Answer practical money questions — bills, invoices, spend, revenue, dates, totals
triggers: [intent_query]
priority: 10
keywords: [total, spend, revenue, how much, sum, balance, cost breakdown]
---

CRITICAL: Answer with REAL data from workspace files. Never copy example values.

WORKFLOW:
  1. Search relevant directories for financial data (use tree to find folders)
  2. Read matching files to extract amounts, dates, line items
  3. Calculate totals/sums explicitly — show your math
  4. Answer with precise numbers + file refs

RULES:
  - ALWAYS include file refs in answer
  - For counting: use grep_count() — one call, exact result
  - For totals: read files, extract amounts, sum explicitly
  - Prefer precision — read actual files, don't guess from filenames
