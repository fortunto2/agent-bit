---
name: purchase-ops
description: Fix purchase processing issues — ID prefix regression
triggers: [intent_edit]
priority: 15
keywords: [purchase, prefix, processing, lane, regression]
---

WORKFLOW:
  1. Read docs/purchase-id-workflow.md for the processing pipeline
  2. List processing/ to find active lanes
  3. Read the active lane configuration
  4. Identify the prefix regression (wrong prefix on downstream processing)
  5. Fix the prefix in the active lane to match documented format
  6. answer(OUTCOME_OK) with refs to docs, processing, and purchase paths

EXAMPLE — Fix purchase prefix:
  read({"path": "docs/purchase-id-workflow.md"}) → prefix format: "PO-YYYY-NNN"
  list({"path": "processing"}) → processing/lane_active.json
  read({"path": "processing/lane_active.json"}) → {"prefix": "PUR-2026-", ...}
  write({"path": "processing/lane_active.json", "content": "{\"prefix\": \"PO-2026-\", ...}"})
  answer({"message": "Fixed purchase prefix from PUR- to PO-", "outcome": "OUTCOME_OK", "refs": ["docs/purchase-id-workflow.md", "processing/lane_active.json"]})

CRITICAL: Read the workflow documentation FIRST — it defines the correct prefix format.
