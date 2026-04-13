---
name: finance-query
description: Answer practical money questions — bills, invoices, spend, revenue, dates, totals
triggers: [intent_query]
priority: 10
keywords: [total, spend, revenue, how much, sum, balance, cost breakdown]
---

CRITICAL: Answer with REAL data from workspace files. Never copy example values.
CRITICAL: NEVER sum numbers in your head — you WILL get it wrong. Use eval() for ALL arithmetic.

WORKFLOW — Revenue/spend by service line:
  1. search("service line keyword", "50_finance/invoices") → find matching invoice files
  2. For date filtering: check filename dates (YYYY_MM_DD prefix) against the requested period
  3. Use eval() to extract amounts and sum:
     eval(code: 'file_paths.filter((p,i) => p >= "50_finance/invoices/2026_03").map((p,i) => { let m = eval("file_"+i).match(/total[:\\s]*([\\d.]+)/i); return m ? parseFloat(m[1]) : 0 }).reduce((a,b)=>a+b,0)', files: ["50_finance/invoices/*"])
  4. Answer with the number only + file refs

WORKFLOW — Bill/purchase lookup:
  1. search("vendor name", "50_finance/purchases") → find matching files
  2. Read the file, extract the specific field (amount, date, line count, etc.)
  3. For "number of lines": count data rows in the table (exclude header/separator)
  4. Answer with precise value + file refs

RULES:
  - ALWAYS use eval() for sums/totals — never mental math
  - ALWAYS include file refs in answer
  - For counting: use search() footer [N matching lines] or eval()
  - Date filtering: compare filename prefix (YYYY_MM_DD) with requested period
