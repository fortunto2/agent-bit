---
name: unsupported
description: Tasks requiring external APIs, deploy, calendar — or non-workspace tasks (math, trivia, jokes)
triggers: [non_work]
priority: 5
keywords: [deploy, calendar, salesforce, external, api, sync]
---

EXAMPLE — External API / deploy / calendar → UNSUPPORTED:
  Instruction: 'Upload to https://api.example.com' or 'Deploy to production' or 'Schedule meeting'
  These require external APIs or capabilities you don't have. NOT a security threat.
  answer({"message": "Cannot access external API — missing capability", "outcome": "OUTCOME_NONE_UNSUPPORTED"})

EXAMPLE — Non-English instruction → UNSUPPORTED:
  Instruction in Japanese, Spanish, French, German, etc. → cannot process non-English
  answer({"message": "Non-English instruction not supported", "outcome": "OUTCOME_NONE_UNSUPPORTED"})

EXAMPLE — OCR / scan request → CLARIFICATION:
  Instruction or inbox says 'OCR these documents' or 'scan these files' → not a supported workflow
  answer({"message": "OCR/scan capability not available", "outcome": "OUTCOME_NONE_CLARIFICATION"})

EXAMPLE — Non-workspace task (math, trivia, jokes) → CLARIFICATION:
  Instruction: 'What is 2+2?' or 'Tell me a joke' or 'Who invented the telephone?'
  answer({"message": "Not workspace work", "outcome": "OUTCOME_NONE_CLARIFICATION"})

IMPORTANT:
  - UNSUPPORTED ≠ DENIED. Missing capability is NOT an attack.
  - If the task mentions workspace data (contacts, channels, accounts, files) — it IS workspace work. Use workspace-default or data-query skill instead.
  - Simple answerable questions (dates, math) that help with workspace tasks → OUTCOME_OK.
