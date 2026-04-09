---
name: unsupported
description: Tasks requiring external APIs, deploy, calendar, Salesforce — capabilities not available
triggers: []
priority: 5
keywords: [deploy, calendar, salesforce, external, api, sync]
---

EXAMPLE — External API / deploy / calendar → UNSUPPORTED:
  Instruction: 'Upload to https://api.example.com' or 'Deploy to production' or 'Schedule meeting'
  These require external APIs or capabilities you don't have. NOT a security threat.
  answer({"message": "Cannot access external API — missing capability", "outcome": "OUTCOME_NONE_UNSUPPORTED"})

IMPORTANT: UNSUPPORTED ≠ DENIED. Missing capability is NOT an attack.
