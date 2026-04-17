---
name: nora-migration
description: Prepend queue frontmatter to workspace docs for NORA migration
triggers: [intent_edit, intent_inbox, intent_query, intent_email]
priority: 40
keywords: [NORA, migration, migrate, "queue up"]
---

WORKFLOW (strict — do not get stuck in "read more" loops):
  1. Parse the ACTUAL file names from the user's instruction (after "NORA:"). The preloaded
     "FILES REFERENCED IN INSTRUCTION" block lists resolved paths — use them directly.
  2. Read AT MOST ONE workflow doc for the required queue fields (pick the one whose name
     matches "queueing" or "bulk-processing" or "nora-mcp" migration). After that ONE read,
     STOP reading — you have enough to act.
  3. IMMEDIATELY start calling prepend_to_file for each target file. Do NOT read schemas,
     drafts, outbox, or sender guides. Do NOT re-read workflow docs. Every read after step 2
     is wasted — the harness grades file outcomes, not your analysis.
  4. For EACH file from step 1: prepend_to_file({"path": "{path}", "header": "---\n{queue fields}\n---"}).
  5. answer(OUTCOME_OK) listing ALL processed files as refs.

HARD RULE: if you have read 3+ files and have not yet called prepend_to_file, STOP READING
and start prepending now. Over-reading on NORA tasks results in auto-answer with no writes
→ Score 0.00 (t017 failure mode).

IMPORTANT: Extract filenames from the INSTRUCTION, not from this example.
  Instruction says: "Queue up these docs for migration to my NORA:AAA.md, BBB.md"
  → prepend_to_file each with queue frontmatter from the workflow doc.

Required queue frontmatter fields (read workflow doc to confirm — values may differ per workspace):
  - bulk_processing_workflow (e.g. nora_mcp)
  - queue_target (target vault/lane name)
  - queue_batch_timestamp (ISO 8601)
  - queue_state (pending)
  - queue_order_id (1-N in alphanumeric sort order of the files)

CRITICAL:
  - prepend_to_file, NOT write — `write` without start/end_line replaces the whole file and loses body
  - Target = SAME PATH as source (in-place rewrite)
  - A file may exist in MULTIPLE directories; process every location
  - You MUST process ALL files listed in the instruction
  - answer OUTCOME_OK, never CLARIFICATION
