# Implementation Plan: Centralized Decision Pipeline

**Track ID:** decision-pipeline_20260405
**Spec:** [spec.md](./spec.md)
**Created:** 2026-04-05
**Status:** [ ] Not Started

## Overview

Extract decision logic from 7 files into `src/pipeline.rs` with typed stages. Each stage: `Input → StageResult<Output>`. Pipeline short-circuits on first `Block`. Existing code moves behind stage interface — no logic rewrite, just reorganization.

## Phase 1: Foundation — StageResult + SenderAssessment

Merge the two sender trust systems and define the pipeline type system.

### Tasks

- [ ] Task 1.1: Create `src/pipeline.rs` with core types:
  ```rust
  pub enum StageResult<T> { Continue(T), Block { outcome: &'static str, message: String } }
  pub struct SenderAssessment { pub trust: SenderTrust, pub domain_match: &'static str, pub reasons: Vec<String> }
  pub struct SecurityAssessment { pub verdict: StageResult<()>, pub ml_label: String, pub ml_conf: f32, pub structural: f32, pub sender: SenderAssessment }
  pub struct PipelineContext { pub instruction: String, pub intent: String, pub inbox_content: String, pub security: Option<SecurityAssessment>, pub crm_graph: CrmGraph }
  ```

- [ ] Task 1.2: Merge `crm_graph.rs::validate_sender()` + `scanner.rs::check_sender_domain_match()` into `pipeline.rs::assess_sender()`. Single function, returns `SenderAssessment`. Inputs: sender email, content, CRM graph, account_domains. Uses strsim for all fuzzy matching, mailparse for email extraction.

- [ ] Task 1.3: Make `validate_sender()` and `check_sender_domain_match()` call `assess_sender()` internally (backward compat wrappers) so existing callers don't break during migration.

### Verification
- [ ] `cargo test` passes (178 tests)
- [ ] `assess_sender()` unit tests: known email → Known, lookalike domain → CrossCompany + mismatch, unknown → Unknown

## Phase 2: Security Scanner — merge scan_inbox + structural guard

Consolidate pre-LLM security checks into one `SecurityScanner::assess()`.

### Tasks

- [ ] Task 2.1: Add `SecurityScanner` struct to `pipeline.rs` with method `assess(content: &str, sender: &SenderAssessment, classifier: &SharedClassifier) -> SecurityAssessment`. Merges:
  - `threat_score()` (HTML injection)
  - `structural_injection_score()` (pattern signals)
  - ML classification (`semantic_classify_inbox_file`)
  - Credential exfiltration detection (OTP + branching logic — currently duplicated in `scan_inbox` lines 156-165 AND `semantic_classify_inbox_file` lines 315-337)
  - Ensemble blocking logic (ML + sender + sensitive data)
  - Structural guard (CROSS_COMPANY + financial)

- [ ] Task 2.2: Add `assess_inbox(pcm, classifier, crm_graph) -> Vec<(path, SecurityAssessment)>` — scans all inbox files, returns per-file assessment. Replaces both `scan_inbox()` and the inline classification in `read_inbox_files()`.

- [ ] Task 2.3: Wire `assess_inbox` into `pregrounding.rs::run_agent()` — replace `scan_inbox()` call + structural guard block + `read_inbox_files()` classification with single `pipeline::assess_inbox()`. Short-circuit on first Block.

### Verification
- [ ] `cargo test` passes
- [ ] t18 still deterministic (CROSS_COMPANY + financial → Block)
- [ ] t19 still passes (known contact → no block)
- [ ] `scan_inbox()` and structural guard code removed from pregrounding.rs

## Phase 3: Pipeline Orchestrator — linear run()

Wire all stages into `DecisionPipeline::run()`.

### Tasks

- [ ] Task 3.1: Create `DecisionPipeline::run()` in `pipeline.rs`:
  ```
  Stage 1: prescan(instruction) → Block | Continue
  Stage 2: classify_instruction(instruction) → Block(injection/non_work) | Continue(label)
  Stage 3: classify_intent(instruction) → intent label
  Stage 4: build_context(pcm, crm_graph) → PipelineContext
  Stage 5: assess_inbox(pcm, classifier, crm_graph) → Block | Continue(enriched inbox)
  Stage 6: assemble_messages(context, hints) → messages for LLM
  Stage 7: run_agent(agent, tools, messages) → (last_msg, history)
  Stage 8: verify_and_submit(pcm, proposed_answer, history) → final answer
  ```
  Each stage logs: `[STAGE:{name}] {result}` for observability.

- [ ] Task 3.2: Refactor `pregrounding.rs::run_agent()` to call `DecisionPipeline::run()`. The 400-line function becomes ~30 lines: build pipeline → run → return result. All hint injection, guard logic, planning skip moves into pipeline stages.

- [ ] Task 3.3: Move hint injection logic (OTP hint, delete hint, query hint, capture-delete hint) into pipeline stage 6 (`assemble_messages`). Currently scattered across pregrounding.rs lines 550-694.

### Verification
- [ ] `cargo test` passes
- [ ] t01, t16, t18 pass on Nemotron
- [ ] `pregrounding.rs::run_agent()` < 50 lines
- [ ] Stage-by-stage log visible in stderr

## Phase 4: Docs & Cleanup

### Tasks

- [ ] Task 4.1: Update CLAUDE.md — replace scattered decision pipeline description with pipeline.rs stage diagram. Update Architecture section.
- [ ] Task 4.2: Remove dead code — unused functions in scanner.rs (old scan_inbox, inline checks), dead wrappers in crm_graph.rs
- [ ] Task 4.3: Update evolve SKILL.md — failure point table references pipeline stages instead of scattered files

### Verification
- [ ] CLAUDE.md reflects centralized pipeline
- [ ] `cargo test` passes, `cargo build` clean
- [ ] No dead code warnings

## Final Verification

- [ ] All acceptance criteria from spec met
- [ ] 178+ tests pass
- [ ] Build clean
- [ ] t01, t16, t18 pass on Nemotron (regression check)
- [ ] Decision log shows stage trace for each trial
- [ ] `pregrounding.rs::run_agent()` < 50 lines
- [ ] No duplicate sender trust / credential exfiltration logic

## Context Handoff

### Session Intent
Consolidate 15 decision points across 7 files into a linear `DecisionPipeline` state machine in `src/pipeline.rs`.

### Key Files
- `src/pipeline.rs` — NEW: DecisionPipeline, StageResult, SenderAssessment, SecurityScanner
- `src/pregrounding.rs` — SIMPLIFY: run_agent() calls pipeline.run(), remove inline guards/hints
- `src/scanner.rs` — EXTRACT: scan_inbox, semantic_classify_inbox_file logic moves to pipeline
- `src/crm_graph.rs` — EXTRACT: validate_sender merges with check_sender_domain_match
- `src/agent.rs` — UNCHANGED: Pac1Agent stays as-is, called from pipeline stage 7
- `src/classifier.rs` — UNCHANGED: classify/classify_intent called from pipeline stages
- `src/tools.rs` — MINOR: guard_content may call pipeline SecurityScanner instead of threat_score directly

### Decisions Made
- **Stages, not middleware**: linear pipeline with short-circuit, not layered middleware. Simpler to reason about, easier to test.
- **StageResult<T> not Result<T>**: Block is not an error — it's a valid outcome. Using custom enum makes the control flow explicit.
- **Backward compat wrappers**: Phase 1 keeps old function signatures working via delegation. Callers migrate gradually in Phase 2-3.
- **No LLM logic change**: agent.rs Pac1Agent, reasoning schema, tool descriptions stay exactly the same. Pipeline only reorganizes pre-LLM and post-LLM decision making.
- **Existing modules as stage impls**: scanner.rs functions become private, called from pipeline stages. No logic rewrite — just moved behind typed interface.

### Risks
- **Phase 2 is the hardest**: merging scan_inbox + structural guard + read_inbox_files has the most entanglement. May need sub-phases.
- **test coverage gap**: some inline guards (OTP hint, CROSS_COMPANY) lack unit tests — adding pipeline stages is a chance to add them.
- **pregrounding.rs is 900+ lines**: simplifying it to ~50 lines is aggressive. Some context assembly (tree, agents.md, contacts summary) may stay in pregrounding, with pipeline handling only decisions.

---
_Generated by /plan. Tasks marked [~] in progress and [x] complete by /build._
