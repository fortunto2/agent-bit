---
name: nora-migration
description: Copy workspace docs in place for NORA migration queue
triggers: [intent_edit, intent_inbox]
priority: 25
keywords: [NORA, migration, migrate]
---

WORKFLOW:
  1. Parse the ACTUAL file names from the user's instruction (after "NORA:"). The preloaded
     "FILES REFERENCED IN INSTRUCTION" block lists resolved paths — use them directly.
  2. Read `99_system/workflows/bulk-processing-and-queueing-frontmatter.md` (or the schema
     doc with the same stem) to learn the REQUIRED queue frontmatter fields for this workspace.
     Without this step you don't know which fields to write, and the harness scores 0.00.
  3. Read `99_system/schemas/finance-record-frontmatter.md` if the schema doc is the spec.
  4. For EACH file: prepend_to_file({"path": "{found_path}", "header": "---\n{queue fields per schema}\n---"})
     — OR if the file already has frontmatter, read it, merge queue fields, then prepend the new frontmatter.
  5. answer(OUTCOME_OK) listing ALL processed files as refs.

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
