---
name: crm-lookup
description: Data queries — find contacts, lookup emails, count entries, search channels
triggers: [intent_query]
priority: 15
---

CRITICAL: Answer with REAL data from the workspace. Never copy example text.

WORKFLOW — Data lookup:
  1. search(pattern, path) to find the file (auto-reads when ≤10 matches)
  2. Extract the EXACT answer from results
  3. answer() with the real data + refs to source files

WORKFLOW — Counting (how many X):
  Use search(pattern, path) — result footer shows [N matching lines]. That number IS the answer.
  Or eval() for complex counting. Do NOT read files and count manually — you WILL miscount.

WORKFLOW — Count projects by status ("how many ACTIVE/STALLED/SIMMERING projects involve X"):
  MUST check BOTH entity link AND status field. Steps:
  1. Find person alias: search(NAME, "10_entities/cast") → read alias field
  2. Use eval to count in ONE call — filter by BOTH entity AND status:
     eval(code: 'var n=0; for(var i=0;i<file_paths.length;i++){var f=eval("file_"+i); if(f.includes("entity.ALIAS") && f.includes("status: `STATUS`")) n++} n', files: ['40_projects/*/README.MD'])
     Replace ALIAS with lowercase alias, STATUS with requested status (active/stalled/simmering/planned).
  3. Answer with the number. If 0 → answer "0".

WORKFLOW — Sum/total queries (how much, total amount):
  1. search for matching files (invoices, bills, purchases)
  2. Read EACH matching file, extract the numeric amount
  3. Use eval() to sum: eval(code: 'files.map(f => parseFloat(f.match(/total.*?(\\d+)/i)?.[1] || 0)).reduce((a,b)=>a+b, 0)')
  4. Return the EXACT number — no rounding, no currency symbol unless asked

WORKFLOW — "Return only number/date/name":
  Answer with EXACTLY what was asked — no extra text. "2" not "2 lines". "April 27, 2026" not "The start date is April 27, 2026".

WORKFLOW — Date-based lookup:
  Use context() to get current date. Calculate target date. Search files by date in filename.
  FOUND → read, answer with refs.
  NOT FOUND → OUTCOME_NONE_CLARIFICATION (not OK, not UNSUPPORTED).

WORKFLOW — Next birthday / DoB lookup:
  1. read_all("10_entities/cast") → extract "Name:MM-DD" for each entity with birthday field
  2. Call date_calc(op: "next_birthday", birthdays: ["Name:MM-DD", "Name2:MM-DD", ...])
     → returns name(s) with next upcoming birthday, sorted alphabetically if tied
  3. Answer with the name(s).
  NEVER compute date comparisons yourself — use date_calc tool.

WORKFLOW — Enumerate all (list ALL X, "in which projects", "which accounts"):
  MUST find ALL matches — missing even ONE = task failure.
  For person-in-projects — ALWAYS use eval with glob (search may miss if >10 matches):
  1. Find person alias: search(NAME, "10_entities/cast") → read alias field (lowercase)
  2. eval(code: 'var r=[]; for(var i=0;i<file_paths.length;i++){var f=eval("file_"+i); if(f.includes("entity.ALIAS")){var m=f.match(/^# (.+)/m); if(m) r.push(m[1])}} r.sort().join("\\n")', files: ['40_projects/*/README.MD'])
  3. Answer with ALL names sorted alphabetically.
  Return human-readable project name from README.MD header, NOT folder path.

IMPORTANT: Always include refs in your answer. Return ONLY the requested data — no explanations.
For project names: return the human-readable name from README.MD (e.g. "Harbor Body"), NOT the folder path.
