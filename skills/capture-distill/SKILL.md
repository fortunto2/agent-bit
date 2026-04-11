---
name: capture-distill
description: Capture from inbox into folders, create distill cards, update threads, delete source
triggers: [intent_inbox, intent_delete]
priority: 15
keywords: [capture, distill, card, thread, delete, remove, clean, clear, discard]
---

WORKFLOW (strict order — do NOT skip steps):
  1. READ inbox file (already loaded above — use the content from context)
  2. WRITE to capture folder: 01_capture/{folder}/{SAME filename as inbox}
  3. WRITE distill card: 02_distill/cards/{SAME filename as inbox}
  4. UPDATE thread in 02_distill/threads/ (read AGENTS.md for rules)
  5. DELETE original inbox file
  6. answer(OUTCOME_OK)

WRONG ORDER: read → delete → answer (SKIPPED writes! This will be BLOCKED.)
CORRECT ORDER: read → write capture → write card → update thread → delete → answer

EXAMPLE — Capture and distill:
  read({"path": "00_inbox/2026-03-01__article-title.md"}) → [inbox content]
  read({"path": "02_distill/cards/_card-template.md"}) → [template]
  write({"path": "01_capture/influential/2026-03-01__article-title.md", "content": "{...captured content}"})
  write({"path": "02_distill/cards/2026-03-01__article-title.md", "content": "{...card from template + source}"})
  read({"path": "02_distill/threads/relevant-thread.md"}) → [existing thread]
  write({"path": "02_distill/threads/relevant-thread.md", "content": "{...updated with new entry}"})
  delete({"path": "00_inbox/2026-03-01__article-title.md"})
  answer({"message": "Captured, created card, updated thread, deleted source", "outcome": "OUTCOME_OK"})

CRITICAL RULES:
  - Keep the EXACT source filename when creating capture and card files. Do NOT rename.
  - ALWAYS delete the inbox file after processing (write BEFORE delete).
  - Thread update is REQUIRED — read 02_distill/AGENTS.md for which thread to update.
  - After reading a file, write IMMEDIATELY. Do NOT re-read — you already have the content.
  - You MUST write ALL 3 files: capture + card + thread. Missing ANY = task failure.
  - If you read the inbox content, you have ALL data needed. No re-reads, just write.

---

DELETE/CLEANUP WORKFLOW (when task is delete-only, NOT capture):
  1. Search to identify target files
  2. Read each candidate to CONFIRM it's the right file
  3. Delete ONLY the confirmed targets
  4. Do NOT create, write, or modify any files

EXAMPLE — Delete with search:
  search({"pattern": "keyword", "path": "contacts"}) → contacts/alice.md
  read({"path": "contacts/alice.md"}) → [confirm this is the target]
  delete({"path": "contacts/alice.md"})
  answer({"message": "Deleted contacts/alice.md", "outcome": "OUTCOME_OK", "refs": ["contacts/alice.md"]})

CRITICAL: DELETE tasks = search + read + delete ONLY. Do NOT write or create files.
  - "Discard" means delete the file. Just call delete() directly.
  - Skip templates (_card-template.md) and README files.
