---
name: cleanup
description: Delete cards, threads, or other files — search first, then delete
triggers: [intent_delete]
priority: 20
keywords: [delete, remove, clean, clear, discard]
---

WORKFLOW:
  For BULK cleanup ("remove all", "clean up all", "delete all cards/threads"):
    1. list() each target directory — get file names
    2. delete() each file directly — do NOT read files first, reading is waste
    3. Skip templates (_card-template.md) and README files
    4. answer() with count and refs

  For TARGETED cleanup ("delete that card", "remove the file"):
    1. search() to identify the target
    2. read() to CONFIRM it's the right file (only when ambiguous)
    3. delete() the confirmed target
    4. answer() with refs

  Do NOT create, write, or modify any files.

EXAMPLE — Bulk cleanup ("remove all captured cards and threads"):
  list({"path": "02_distill/cards"}) → card_001.md, card_002.md, _card-template.md
  list({"path": "02_distill/threads"}) → thread_001.md, thread_002.md
  delete({"path": "02_distill/cards/card_001.md"})
  delete({"path": "02_distill/cards/card_002.md"})
  delete({"path": "02_distill/threads/thread_001.md"})
  delete({"path": "02_distill/threads/thread_002.md"})
  answer({"message": "Deleted 2 cards and 2 threads", "outcome": "OUTCOME_OK", "refs": ["02_distill/cards/card_001.md", "02_distill/cards/card_002.md", "02_distill/threads/thread_001.md", "02_distill/threads/thread_002.md"]})
  NOTE: Do NOT read() files before deleting in bulk ops. Skip _card-template.md (template).

EXAMPLE — Targeted delete ("delete that card about project X"):
  search({"pattern": "project X", "path": "02_distill/cards"}) → card_005.md
  read({"path": "02_distill/cards/card_005.md"}) → [confirm this is the target]
  delete({"path": "02_distill/cards/card_005.md"})
  answer({"message": "Deleted card_005.md", "outcome": "OUTCOME_OK", "refs": ["02_distill/cards/card_005.md"]})

CRITICAL RULES:
  - DELETE tasks = list + delete + answer ONLY. Do NOT write, create, or capture files.
  - BULK = "remove all" / "clean up all" → list then delete each. NEVER read() files in bulk ops.
  - NEVER write("") or write empty content before delete. Just call delete() directly.
  - "Discard" means delete the file. NOT write empty then delete — just delete.
  - "cards and threads" = ONLY 02_distill/cards/ and 02_distill/threads/
  - Do NOT delete from 01_capture/ — captured sources are IMMUTABLE
  - Do NOT delete from 00_inbox/ — inbox files are separate
  - Skip templates (_card-template.md) and README files
  - When task is ambiguous, search first to identify the exact target.
