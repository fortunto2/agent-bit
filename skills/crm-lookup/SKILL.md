---
name: crm-lookup
description: Data queries — find contacts, lookup emails, count entries, search channels
triggers: [intent_query]
priority: 15
---

CRITICAL: Answer with REAL data from the workspace. Never copy example text.

WORKFLOW — Data lookup:
  1. search or search_and_read to find the file
  2. Read the file, extract the EXACT answer
  3. answer() with the real data + refs to source files

WORKFLOW — Counting (how many X):
  Use grep_count(pattern, path) for exact count — one tool call.
  Or use search — result shows [N matching lines]. That number IS the answer.
  Do NOT read files and count manually — you WILL miscount.

WORKFLOW — Date-based lookup:
  Use context() to get current date. Calculate target date. Search files by date in filename.
  FOUND → read, answer with refs.
  NOT FOUND → OUTCOME_NONE_CLARIFICATION (not OK, not UNSUPPORTED).

WORKFLOW — Enumerate all (list ALL X, "in which projects", "which accounts"):
  MUST use search with BROAD pattern to find ALL matches — NOT just the first one.
  1. search(pattern: "keyword", path: "dir/") → shows ALL matching files
  2. If result says "[N matching lines]" with N > 1: read EACH matching file
  3. Return ALL matches sorted alphabetically — missing even ONE = task failure
  Alternative: eval() with glob to read ALL files at once:
  eval(code: 'file_paths.filter((p,i) => eval("file_"+i).includes("keyword"))', files: ['dir/*/README.MD'])
  For project names: extract human-readable name from README, not folder name.

IMPORTANT: Always include refs in your answer. Return ONLY the requested data — no explanations.
For project names: return the human-readable name from README.MD (e.g. "Harbor Body"), NOT the folder path.
