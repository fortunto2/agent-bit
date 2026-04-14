---
name: nora-migration
description: Copy workspace docs in place for NORA migration queue
triggers: [intent_edit, intent_inbox]
priority: 25
keywords: [NORA, migration, migrate]
---

WORKFLOW:
  1. Parse the ACTUAL file names from the user's instruction (after "NORA:")
  2. search({"pattern": "name1|name2|name3"}) — use stems from instruction, NOT from this example
  3. For EACH found file: copy_file({"source": "{found_path}", "target": "{found_path}"})
  4. answer(OUTCOME_OK) listing ALL copied files as refs

IMPORTANT: Extract filenames from the INSTRUCTION, not from this example.
  Instruction says: "Queue up these docs for migration to my NORA:AAA.md, BBB.md"
  → search({"pattern": "AAA|BBB"})
  → copy_file for each result

CRITICAL:
  - Use copy_file — preserves full content byte-for-byte (no truncation)
  - Target = SAME PATH as source (in-place rewrite)
  - A file may exist in MULTIPLE directories (30_knowledge/notes/ AND 99_system/schemas/ etc.)
    You MUST copy_file for EVERY location found — not just the first match
  - If a filename matches in 2+ places, copy ALL of them
  - If source file NOT found anywhere: create at 30_knowledge/notes/{filename}:
    write({"path": "30_knowledge/notes/{filename}", "content": "# {Title}\n\n(queued for NORA migration)\n"})
  - You MUST process ALL files listed in the instruction
  - answer OUTCOME_OK, never CLARIFICATION
