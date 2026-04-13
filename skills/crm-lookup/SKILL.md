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

WORKFLOW — Enumerate all (list ALL X, "in which projects", "which accounts"):
  MUST use search with BROAD pattern to find ALL matches — NOT just the first one.
  For person-in-projects: search(pattern: "entity.ALIAS", path: "40_projects") — linked_entities uses lowercase alias.
  To find alias: search(pattern: "NAME", path: "10_entities/cast") → read alias field → use in project search.
  Search auto-reads ≤10 matches — extract human-readable project name from README.MD header, NOT folder path.
  Return ALL matches sorted alphabetically — missing even ONE = task failure.
  Alternative: eval() with glob to read ALL files at once:
  eval(code: 'file_paths.filter((p,i) => eval("file_"+i).includes("entity.NAME"))', files: ['40_projects/*/README.MD'])

IMPORTANT: Always include refs in your answer. Return ONLY the requested data — no explanations.
For project names: return the human-readable name from README.MD (e.g. "Harbor Body"), NOT the folder path.
