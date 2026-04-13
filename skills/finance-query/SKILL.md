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
  2. For date filtering: compare filename prefix (YYYY_MM_DD) with requested start date
  3. CRITICAL: extract the LINE ITEM subtotal for the specific service line, NOT the invoice total!
     Invoice may have multiple line items — only sum the row matching the service line name.
  4. Use eval() to extract per-line-item amounts and sum:
     eval(code: 'var sum=0; for(var i=0;i<file_paths.length;i++){if(file_paths[i]>="50_finance/invoices/YYYY_MM"){var f=eval("file_"+i); var lines=f.split("\\n"); for(var j=0;j<lines.length;j++){if(lines[j].toLowerCase().includes("SERVICE_LINE_KEYWORD")){var nums=lines[j].match(/(\\d+)\\s*\\|\\s*$/); if(nums) sum+=parseInt(nums[1])}}}} sum', files: ["50_finance/invoices/*"])
     Replace YYYY_MM with start date prefix, SERVICE_LINE_KEYWORD with lowercase service line.
  5. Answer with the number only + file refs

WORKFLOW — Bill/purchase lookup:
  1. search("vendor name", "50_finance/purchases") OR read the specific file if path given
  2. Read the file, extract the specific field (amount, date, quantity, line count, etc.)
  3. For "number of lines" or "how many items": count ONLY data rows in the table body.
     Exclude: header row, separator lines (---), notes, totals, empty lines. Count ONLY item/product rows.
  4. For "quantity of X": find the specific line item row, return the quantity column value.
  5. For "total amount": read the total/sum field, NOT individual line amounts.
  6. Answer with precise value + file refs

WORKFLOW — "How much did I pay VENDOR in total?" (aggregate across multiple bills):
  1. search("vendor name", "50_finance/purchases") → find ALL bills from that vendor
  2. Read EACH bill, extract the total/amount field
  3. Use eval() to sum ALL totals: eval(code: 'amounts.reduce((a,b)=>a+b,0)', ...)
  4. Answer with the grand total number + refs to ALL bills

WORKFLOW — "How much did VENDOR charge for LINE ITEM N days ago?":
  1. Calculate target date: use date_calc(op: "add_days", date: "today", days: -N)
  2. search("vendor name", "50_finance/purchases") → find bills matching date
  3. Read the bill, find the specific line item row, return its amount
  4. Answer with the exact amount number

WORKFLOW — "Looking at this bill, how much did I pay in total?" (bill path given):
  1. Read the given bill file → extract vendor name from content
  2. search("vendor name", "50_finance/purchases") → find ALL bills from that vendor  
  3. Use eval() to sum totals from ALL matching bills:
     eval(code: 'var sum=0; for(var i=0;i<file_paths.length;i++){var f=eval("file_"+i); var m=f.match(/total[_eur]*\\s*\\|\\s*(\\d+)/i); if(m) sum+=parseInt(m[1])} sum', files: ["50_finance/purchases/*vendor*"])
  4. Answer with the grand total number

RULES:
  - ALWAYS use eval() for sums/totals — never mental math. You WILL get it wrong without eval.
  - ALWAYS include file refs in answer
  - For "N days ago": use date_calc tool first, then filter files by date
  - Date filtering: compare filename prefix (YYYY_MM_DD) with requested period
  - For service line revenue: sum LINE ITEM amounts (last column in table row), NOT invoice totals
