---
name: nora-migration
description: Copy workspace docs to knowledge notes for NORA migration
triggers: [intent_edit, intent_inbox]
priority: 25
keywords: [NORA, migration, migrate]
---

WORKFLOW:
  1. Parse file list from instruction: "Queue up these docs for migration to my NORA: file1.md, file2.md, ..."
  2. Find ALL source files in one batch:
     search({"pattern": "file1|file2|file3"}) — pipe-separated, searches entire workspace
  3. read each found file
  4. For EACH file: write FULL verbatim content to THE SAME PATH where it was found
     (if file is at 99_system/workflows/sending-email.md → write to 99_system/workflows/sending-email.md)
  5. answer(OUTCOME_OK) with refs to ALL written files

EXAMPLE — Queue 3 docs:
  Instruction: "Queue up these docs for migration to my NORA: sending-email.md, parking-lot.md, design-constraints.md"

  search({"pattern": "sending-email|parking-lot|design-constraints"})
    → 99_system/workflows/sending-email.md
    → 30_knowledge/notes/parking-lot.md
    → 30_knowledge/notes/design-constraints.md

  read({"path": "99_system/workflows/sending-email.md"}) → full content
  read({"path": "30_knowledge/notes/parking-lot.md"}) → full content
  read({"path": "30_knowledge/notes/design-constraints.md"}) → full content

  write({"path": "99_system/workflows/sending-email.md", "content": "...FULL verbatim..."})
  write({"path": "30_knowledge/notes/parking-lot.md", "content": "...FULL verbatim..."})
  write({"path": "30_knowledge/notes/design-constraints.md", "content": "...FULL verbatim..."})

  answer({"message": "Queued 3 docs for NORA migration", "outcome": "OUTCOME_OK", "refs": [...]})

CRITICAL:
  - Write to the SAME PATH where the file was found — do NOT move files to a different directory
  - Copy FULL content verbatim — do NOT summarize, truncate, or add frontmatter
  - Source files can be ANYWHERE: 99_system/, 30_knowledge/, other dirs — search broadly
  - If source file NOT found anywhere: create at 30_knowledge/notes/{filename} with title as heading:
    write({"path": "30_knowledge/notes/{filename}", "content": "# {Title From Filename}\n\n(queued for NORA migration)\n"})
  - You MUST write ALL requested files — never skip, never CLARIFICATION
  - This is a REAL workspace task — answer OUTCOME_OK, never CLARIFICATION
