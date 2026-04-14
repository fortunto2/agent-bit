---
name: nora-migration
description: Copy workspace docs in place for NORA migration queue
triggers: [intent_edit, intent_inbox]
priority: 25
keywords: [NORA, migration, migrate]
---

WORKFLOW:
  1. Parse file list from instruction: "Queue up these docs for migration to my NORA: file1.md, file2.md, ..."
  2. Find ALL source files in one batch:
     search({"pattern": "file1|file2|file3"}) — pipe-separated, searches entire workspace
  3. For EACH file: copy_file to THE SAME PATH (in-place rewrite marks it for migration)
     copy_file({"source": "99_system/workflows/sending-email.md", "target": "99_system/workflows/sending-email.md"})
  4. answer(OUTCOME_OK) with refs to ALL copied files

EXAMPLE — Queue 3 docs:
  Instruction: "Queue up these docs for migration to my NORA: sending-email.md, parking-lot.md, design-constraints.md"

  search({"pattern": "sending-email|parking-lot|design-constraints"})
    → 99_system/workflows/sending-email.md
    → 30_knowledge/notes/parking-lot.md
    → 30_knowledge/notes/design-constraints.md

  copy_file({"source": "99_system/workflows/sending-email.md", "target": "99_system/workflows/sending-email.md"})
  copy_file({"source": "30_knowledge/notes/parking-lot.md", "target": "30_knowledge/notes/parking-lot.md"})
  copy_file({"source": "30_knowledge/notes/design-constraints.md", "target": "30_knowledge/notes/design-constraints.md"})

  answer({"message": "Queued 3 docs for NORA migration", "outcome": "OUTCOME_OK", "refs": ["99_system/workflows/sending-email.md", "30_knowledge/notes/parking-lot.md", "30_knowledge/notes/design-constraints.md"]})

CRITICAL:
  - Use copy_file — preserves full content byte-for-byte (no truncation)
  - Target = SAME PATH as source (in-place rewrite)
  - Source files can be ANYWHERE: 99_system/, 30_knowledge/, other dirs — search broadly
  - If source file NOT found: create at 30_knowledge/notes/{filename} with title as heading:
    write({"path": "30_knowledge/notes/{filename}", "content": "# {Title From Filename}\n\n(queued for NORA migration)\n"})
  - You MUST process ALL requested files — never skip, never CLARIFICATION
  - This is a REAL workspace task — answer OUTCOME_OK, never CLARIFICATION
