---
name: crm-lookup
description: Data queries — find contacts, lookup emails, count entries, search channels, birthdays
triggers: [intent_query]
priority: 15
keywords: [birthday, born, DoB, how many, which projects, start date, quote, message]
---

CRITICAL: Answer with REAL data from the workspace. Never copy example text.

WORKFLOW — Data lookup:
  1. search(pattern, path) to find the file (auto-reads when ≤10 matches)
  2. Extract the EXACT answer from results
  3. answer() with the real data + refs to source files

WORKFLOW — Epithet/descriptor resolution ("my daughter", "our PM", "the client at the tax firm", "the 3D printer"):
  Instruction may reference a person or thing by role, relationship, or description
  instead of a name. Resolve via `10_entities/cast/` (people) and `10_entities/` (gear/things):
    1. read_all("10_entities/cast") → scan frontmatter for role/relationship/description
    2. If not a person: read_all("10_entities") for gear/equipment/shared-resource files
    3. Match the epithet to an entity alias, then use that alias for downstream queries
  Examples: "my daughter" → role: family → alias=juniper; "our PM" → role: project-manager → alias=sara;
  "the 3D printer" → 10_entities/gear/*.md → alias=voron_3d. Do NOT CLARIFICATION on an
  epithet without trying to resolve it via cast/entities first.

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

WORKFLOW — Birthday (next birthday, DoB, "born", "when was X born"):
  CRITICAL: NEVER compare dates in your head — you WILL get it wrong. Use date_calc.
  1. read_all("10_entities/cast") → for each file with `birthday:` field, extract "Name:MM-DD"
  2. date_calc(op: "next_birthday", birthdays: ["Sara Demir:01-26", "Petra Novak:06-07", ...])
     → returns the name(s) whose birthday comes next after workspace date
  3. For "when was X born" / DoB: just read the entity file and return the birthday field.
     Use date_calc(op: "format", date: "YYYY-MM-DD", output_format: "DD-MM-YYYY") to reformat if needed.

WORKFLOW — Enumerate all (list ALL X, "in which projects", "which accounts"):
  MUST find ALL matches — missing even ONE = task failure.
  For person-in-projects — ALWAYS use eval with glob (search may miss if >10 matches):
  1. Find person alias: search(NAME, "10_entities/cast") → read alias field (lowercase)
  2. eval(code: 'var r=[]; for(var i=0;i<file_paths.length;i++){var f=eval("file_"+i); if(f.includes("entity.ALIAS")){var m=f.match(/^# (.+)/m); if(m) r.push(m[1])}} r.sort().join("\\n")', files: ['40_projects/*/README.MD'])
  3. Answer with ALL names sorted alphabetically.
  Return human-readable project name from README.MD header, NOT folder path.

IMPORTANT: Always include refs in your answer. Return ONLY the requested data — no explanations.
For project names: return the human-readable name from README.MD (e.g. "Harbor Body"), NOT the folder path.
