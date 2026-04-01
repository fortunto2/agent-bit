# Implementation Plan: SGR Hooks + Plan→Execute Pipeline + Fuzzy Search

**Track ID:** sgr-hooks-pipeline_20260401
**Spec:** [spec.md](./spec.md)
**Created:** 2026-04-01
**Status:** [ ] Not Started

## Overview
3 phases: first move existing logic into proper sgr-agent hooks, then add Plan→Execute pipeline using PlanningAgent, then add strsim fuzzy search. Each phase is independent — hooks improve existing flow, pipeline adds new capability, fuzzy improves search quality.

## Phase 1: Activate Agent Hooks <!-- checkpoint:d3bdb75 -->
Move scattered logic (action ledger, tool routing, security checks) into the framework's extension points. No behavior change — same logic, proper architecture.

### Tasks
- [x] Task 1.1: Implement `after_action()` in Pac1Agent (`src/agent.rs`). Move action ledger recording from LoopEvent callback in `src/main.rs` into the hook. The hook receives `(ctx, tool_name, output)` — record `"[{step}] {tool_name} → {output_preview}"` into the ledger. Remove the `agent.record_action()` call from the LoopEvent::ToolResult callback. <!-- sha:82b87e2 -->
- [x] Task 1.2: Implement `prepare_context()` in Pac1Agent (`src/agent.rs`). Inject step_count + action ledger into `ctx.custom` for external consumers. Nudge stays in decide_stateful (Agent trait's decide_stateful doesn't receive ctx — framework limitation). <!-- sha:82b87e2 -->
- [x] Task 1.3: Router stays inline in decide_stateful — it needs task_type from Phase 1 reasoning, but prepare_tools runs BEFORE decide (Agent trait limitation). prepare_tools returns all tools; fine-grained routing remains after reasoning. <!-- sha:82b87e2 -->
- [x] Task 1.4: Move post-read security check into `after_action()`. After tool execution, if tool is "read" or "search", run `structural_injection_score()` on output. If score ≥ 0.30, inject a warning into `ctx.custom["security_warning"]` that decide_stateful reads. <!-- sha:d3bdb75 -->

### Verification
- [x] action ledger still records tool calls (via after_action hook)
- [x] tool routing unchanged — same tools exposed per task_type (inline in decide_stateful)
- [x] security warnings in after_action for suspicious read/search output
- [x] cargo test passes, 69 tests, no regression

## Phase 2: Plan→Execute Pipeline <!-- checkpoint:b955ffa -->
Add PlanningAgent phase before Pac1Agent execution. PlanningAgent reads tree/inbox/README in ≤5 steps, produces structured Plan with per-step tool_hints. Pac1Agent then follows the plan.

### Tasks
- [x] Task 2.1: Add `strsim = "0.11"` to `Cargo.toml` dependencies. Add `sgr_agent::agents::planning::{PlanningAgent, Plan}` + `PlanTool` import to `src/main.rs`. <!-- sha:6288dc1 -->
- [x] Task 2.2: Create planning system prompt in `src/main.rs`. <!-- sha:b955ffa -->
- [x] Task 2.3: Add `run_planning_phase()` function in `src/main.rs`. <!-- sha:b955ffa -->
- [x] Task 2.4: Integrate planning phase into `run_agent()`. <!-- sha:b955ffa -->
- [x] Task 2.5: Plan injected as system message — model reads tool_hints naturally. No prepare_tools change needed (decide_stateful can't read ctx). <!-- sha:b955ffa -->

### Verification
- [x] Planning phase completes in ≤5 steps (t01: 3 planning steps, t03: 3 steps)
- [x] Plan contains 2-5 steps with tool_hints
- [x] Main agent receives plan as context (system message)
- [x] t01 passes with planning (1.0, 3 exec steps)
- [x] cargo test passes (69 tests)

## Phase 3: strsim Fuzzy Search <!-- checkpoint:305b40a -->
Add Levenshtein distance matching to SearchTool and CrmGraph for better name resolution.

### Tasks
- [x] Task 3.1: Add strsim Levenshtein to `smart_search()` in `src/tools.rs`. <!-- sha:305b40a -->
- [x] Task 3.2: Add `fuzzy_find_contact()` to `src/crm_graph.rs`. <!-- sha:305b40a -->
- [x] Task 3.3: Integrate `fuzzy_find_contact()` into `validate_sender()` in `src/crm_graph.rs`. <!-- sha:305b40a -->

### Verification
- [x] "Jon Smith" fuzzy-matches "john smith" (>0.7 Levenshtein) — test passes
- [x] Unknown sender with close name match gets Plausible trust
- [x] "Completely Different Name" returns None — no false positive
- [x] cargo test passes (73 tests, 4 new fuzzy)

## Phase 4: Benchmark + Docs

### Tasks
- [ ] Task 4.1: Run 8-task quick sample on Nemotron, verify no regression
- [ ] Task 4.2: Update CLAUDE.md — document hooks, planning pipeline, fuzzy search
- [ ] Task 4.3: Remove dead code — any inline logic that was moved to hooks

### Verification
- [ ] Nemotron sample ≥ 6/8
- [ ] CLAUDE.md reflects current architecture
- [ ] cargo build + cargo test clean

## Final Verification
- [ ] All acceptance criteria from spec met
- [ ] No task-specific patterns or task IDs in code
- [ ] cargo test passes
- [ ] Build succeeds
- [ ] Documentation up to date

## Context Handoff

### Session Intent
Activate sgr-agent hooks, add Plan→Execute pipeline, add strsim fuzzy search — all universal improvements that leverage the framework properly.

### Key Files
- `src/agent.rs` — Pac1Agent: implement prepare_context, prepare_tools, after_action hooks
- `src/main.rs` — run_agent: add planning phase, move ledger/nudge to hooks, remove LoopEvent record_action
- `src/tools.rs` — SearchTool: add strsim fallback in smart_search
- `src/crm_graph.rs` — CrmGraph: add fuzzy_find_contact with Levenshtein
- `Cargo.toml` — add strsim dep

### Decisions Made
- Sequential pipeline (2 run_loops) over SwarmManager — simpler, no concurrency issues, same thread
- PlanningAgent wrapping Pac1Agent over custom planner — reuses existing read-only enforcement
- strsim as direct dep (not via sgr-agent) — sgr-agent doesn't re-export it
- Max 5 planning steps — PAC1 tasks are short, planning shouldn't eat the budget
- Hooks over inline logic — proper framework extension, cleaner code, no behavior change

### Risks
- Planning phase adds 5 extra LLM calls — ~25% latency increase on simple tasks. Mitigation: if plan returns None (submit_plan not called), skip injection
- prepare_tools hook changes tool visibility — subtle behavior changes possible. Mitigation: exact same router logic, just moved
- strsim false positives on short names — "Al" matches "Ali" etc. Mitigation: min name length 3, score threshold 0.7

---
_Generated by /plan. Tasks marked [~] in progress and [x] complete by /build._
