# Specification: Centralized Decision Pipeline

**Track ID:** decision-pipeline_20260405
**Type:** Refactor
**Created:** 2026-04-05
**Status:** Draft

## Summary

Consolidate 15 scattered decision points across 7 files into a single linear `DecisionPipeline` state machine in a new `src/pipeline.rs`. Currently, the "DENIED or OK" decision is made partially in scanner.rs (prescan, ensemble), crm_graph.rs (sender trust), pregrounding.rs (structural guards, hints), agent.rs (LLM reasoning, tool filtering), tools.rs (post-read guard, OutcomeValidator), and prompts.rs (decision tree text). Each layer has incomplete information and duplicates logic from other layers (e.g., sender trust checked in both `validate_sender` and `check_sender_domain_match`; credential exfiltration detected in both `scan_inbox` and `semantic_classify_inbox_file`).

The refactored pipeline: each stage takes structured input, produces structured output, short-circuits on Block, and passes enriched context to the next stage. Every decision logged with stage name for observability. Existing modules become stage implementations — no logic rewritten, just moved behind a unified interface.

## Acceptance Criteria

- [ ] New `src/pipeline.rs` with `DecisionPipeline` struct and `run()` method that orchestrates all stages
- [ ] `SenderAssessment` struct merges `validate_sender()` + `check_sender_domain_match()` — single source of truth for sender trust
- [ ] `SecurityScanner::assess()` merges threat_score + structural_injection_score + ML classification + sender trust into one call returning `SecurityAssessment { verdict, scores, reasons }`
- [ ] Each stage returns `StageResult<T> = Continue(T) | Block(outcome, message)` — first Block short-circuits pipeline
- [ ] `pregrounding.rs::run_agent()` reduced to: build context → `pipeline.run()` → submit answer. No inline guards or scattered checks.
- [ ] All 178 existing tests pass
- [ ] No regression on t01, t18 (deterministic), t16 (query)
- [ ] Decision log shows stage-by-stage trace: `[STAGE:prescan] Pass`, `[STAGE:ensemble] Block: DENIED`

## Dependencies

- No new crates — uses existing strsim, mailparse, ort, petgraph
- Depends on: ML intent classification (done), strsim domain matching (done), lookalike detection (done)

## Out of Scope

- NLI cross-encoder (separate track, plugs into SecurityScanner later)
- Changing LLM reasoning schema (agent.rs Pac1Agent stays as-is)
- Rewriting OutcomeValidator or Verifier — just moving behind pipeline interface
- Changing any scoring logic — only reorganizing where decisions live

## Technical Notes

- **Duplication found**: sender trust (crm_graph + scanner), credential exfiltration (scan_inbox + semantic_classify), HTML injection (threat_score called from 3 places)
- **Key insight**: scanner.rs `scan_inbox` and pregrounding.rs structural guard do the same thing (Block before LLM) but with different inputs. Should be one stage.
- **Existing modules become stage impls**: scanner.rs → `stage_prescan()` + `stage_ensemble()`, crm_graph.rs → `stage_sender_trust()`, classifier.rs → `stage_intent()`, agent.rs → unchanged (called from pipeline)
- **strsim already available** for fuzzy matching, **mailparse** for email parsing — both already used, just need consistent application
