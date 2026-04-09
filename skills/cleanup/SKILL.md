---
name: cleanup
description: Delete cards, threads, or other files — search first, then delete
triggers: [intent_delete]
priority: 10
keywords: [delete, remove, clean, clear]
---

WORKFLOW:
  1. Search to identify target files (cards, threads, etc.)
  2. Read each candidate to CONFIRM it's the right file
  3. Delete ONLY the confirmed targets
  4. Do NOT create, write, or modify any files

EXAMPLE — Delete with ambiguous reference ("delete that card", "remove the file"):
  search({"pattern": "keyword from context", "path": "contacts"}) → contacts/alice.md, contacts/bob.md
  read({"path": "contacts/alice.md"}) → [confirm this is the target]
  delete({"path": "contacts/alice.md"})
  answer({"message": "Deleted contacts/alice.md", "outcome": "OUTCOME_OK", "refs": ["contacts/alice.md"]})

EXAMPLE — Bulk cleanup ("remove all cards and threads"):
  list({"path": "02_distill/cards"}) → [list of card files]
  list({"path": "02_distill/threads"}) → [list of thread files]
  Delete each file one by one (skip templates like _card-template.md).
  answer({"message": "Deleted N cards and M threads", "outcome": "OUTCOME_OK"})

CRITICAL RULES:
  - DELETE tasks = search + read + delete ONLY. Do NOT write, create, or capture files.
  - Skip templates and README files when doing bulk cleanup.
  - When task is ambiguous, search first to identify the exact target.
