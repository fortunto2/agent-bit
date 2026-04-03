# Benchmark: nemotron-120b @ b3ec68e

**Date:** 2026-04-03
**Provider:** nemotron (CF Workers AI Gateway)
**Model:** nemotron-3-120b-a12b
**Commit:** b3ec68e (SDK migration + prompt restore + t08/t12 fix)
**Agent:** Pac1Agent (prompt_mode=explicit)
**Score:** 16/22 = **72.7%** (8 tasks not in run)

## Per-Task Results

| Task | Score | Notes |
|------|-------|-------|
| t01 | 1.00 | |
| t02 | 1.00 | |
| t04 | 0.00 | non-deterministic |
| t08 | 0.00 | non-deterministic (2/3 on GPT-5.4) |
| t09 | 1.00 | |
| t10 | 1.00 | |
| t11 | 1.00 | |
| t12 | 1.00 | FIXED: auto-answer → CLARIFICATION |
| t13 | 1.00 | |
| t14 | 1.00 | |
| t15 | 1.00 | |
| t16 | 1.00 | |
| t17 | 1.00 | |
| t20 | 1.00 | |
| t21 | 1.00 | |
| t22 | 1.00 | |
| t23 | 0.00 | non-deterministic (~75%) |
| t24 | 0.00 | non-deterministic |
| t25 | 0.00 | non-deterministic |
| t26 | 0.00 | non-deterministic |
| t27 | 1.00 | |
| t28 | 1.00 | |

## Session Changes (2026-04-02 to 2026-04-03)

- bitgn-sdk: typed Connect-RPC client (published crates.io v0.2.0)
- pcm.rs + bitgn.rs migrated to SDK (no more hand-rolled JSON)
- schemars: all 11 tool schemas auto-derived
- Prompt-to-tools: enriched tool descriptions, dynamic example injection
- Decision tree restored for explicit mode (weak models need it)
- t08: validate OK-but-not-completed answers
- t12: auto-answer fallback → CLARIFICATION
- Workspace setup with bitgn-sdk as member crate
