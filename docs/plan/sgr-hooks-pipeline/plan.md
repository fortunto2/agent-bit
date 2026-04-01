# Implementation Plan: SGR Hooks + Plan→Execute Pipeline + Fuzzy Search

**Track ID:** sgr-hooks-pipeline_20260401
**Spec:** [spec.md](./spec.md)
**Created:** 2026-04-01
**Status:** [ ] Not Started

## Overview
3 phases: first move existing logic into proper sgr-agent hooks, then add Plan→Execute pipeline using PlanningAgent, then add strsim fuzzy search. Each phase is independent — hooks improve existing flow, pipeline adds new capability, fuzzy improves search quality.

## Phase 1: Activate Agent Hooks
Move scattered logic (action ledger, tool routing, security checks) into the framework's extension points. No behavior change — same logic, proper architecture.

### Tasks
- [ ] Task 1.1: Implement `after_action()` in Pac1Agent (`src/agent.rs`). Move action ledger recording from LoopEvent callback in `src/main.rs` into the hook. The hook receives `(ctx, tool_name, output)` — record `"[{step}] {tool_name} → {output_preview}"` into the ledger. Remove the `agent.record_action()` call from the LoopEvent::ToolResult callback.
- [ ] Task 1.2: Implement `prepare_context()` in Pac1Agent (`src/agent.rs`). Inject action ledger text and adaptive nudge into `ctx.custom` so they're available in `decide_stateful()`. Move the ledger injection and nudge logic from the beginning of `decide_stateful()` into this hook. Read from `ctx.custom` in decide_stateful instead.
- [ ] Task 1.3: Implement `prepare_tools()` in Pac1Agent (`src/agent.rs`). Move the router logic (security→answer only, search→read tools, analyze→read-first-then-full) from inline in `decide_stateful()` into this hook. Return filtered tool names based on `step_count` and last `task_type` (store in ctx.custom). The loop will pass filtered ToolRegistry to decide_stateful.
- [ ] Task 1.4: Move post-read security check into `after_action()`. After tool execution, if tool is "read" or "search", run `structural_injection_score()` on output. If score ≥ 0.30, inject a warning into `ctx.custom["security_warning"]` that decide_stateful reads.

### Verification
- [ ] action ledger still records tool calls (visible in PAC1_DEBUG)
- [ ] tool routing unchanged — same tools exposed per task_type
- [ ] security warnings still appear for suspicious content
- [ ] cargo test passes, no regression

## Phase 2: Plan→Execute Pipeline
Add PlanningAgent phase before Pac1Agent execution. PlanningAgent reads tree/inbox/README in ≤5 steps, produces structured Plan with per-step tool_hints. Pac1Agent then follows the plan.

### Tasks
- [ ] Task 2.1: Add `strsim = "0.11"` to `Cargo.toml` dependencies. Add `sgr_agent::agents::planning::{PlanningAgent, Plan, PlanStep}` import to `src/main.rs`.
- [ ] Task 2.2: Create planning system prompt in `src/main.rs`. Concise prompt: "You are a CRM task planner. Read the file tree, inbox, and README files. Then call submit_plan with steps. Each step: description + tool_hints (which tools to use). Common patterns: search→read→answer for lookups, read inbox→classify→answer for security, read→write→answer for edits."
- [ ] Task 2.3: Add `run_planning_phase()` function in `src/main.rs`. Creates PlanningAgent wrapping a Pac1Agent (read-only mode), registers read-only PCM tools + `submit_plan` tool from sgr-agent. Runs `run_loop()` with max_steps=5. Extracts `Plan::from_context()`. Returns `Option<Plan>`.
- [ ] Task 2.4: Integrate planning phase into `run_agent()`. After pre-scan but before main agent loop: call `run_planning_phase()`. If Plan returned, inject `plan.to_message()` as system message into the main agent's messages. Log plan steps to stderr.
- [ ] Task 2.5: Use plan's `tool_hints` in `prepare_tools()`. If ctx has a plan, and current step maps to a plan step, prefer that step's tool_hints for tool filtering. Fall back to router logic if no plan or no hints.

### Verification
- [ ] Planning phase completes in ≤5 steps
- [ ] Plan contains 2-5 steps with tool_hints
- [ ] Main agent receives plan as context
- [ ] Simple tasks (t01, t09, t16) still pass — plan doesn't slow them down
- [ ] cargo test passes

## Phase 3: strsim Fuzzy Search
Add Levenshtein distance matching to SearchTool and CrmGraph for better name resolution.

### Tasks
- [ ] Task 3.1: Add strsim Levenshtein to `smart_search()` in `src/tools.rs`. After fuzzy_regex fails, try Levenshtein distance matching: read CRM contacts directory listing, score each filename against query with `strsim::normalized_levenshtein()`, if best score > 0.7 read that file. This catches typos that regex wildcards miss (e.g. "Schmitt" vs "Schmidt").
- [ ] Task 3.2: Add `fuzzy_find_contact()` to `src/crm_graph.rs`. Given a name query, iterate graph nodes, compute `strsim::normalized_levenshtein()` against each contact name. Return best match if score > 0.7. Use for sender validation when exact match fails.
- [ ] Task 3.3: Integrate `fuzzy_find_contact()` into `validate_sender()` in `src/crm_graph.rs`. When email domain lookup returns Unknown, try fuzzy name match against sender name. If found, upgrade trust to Plausible.

### Verification
- [ ] "Schmitt" fuzzy-matches "Schmidt" in contacts (>0.7 Levenshtein)
- [ ] Unknown sender with close name match gets Plausible trust
- [ ] No false positives on very different names (<0.5 score)
- [ ] cargo test passes with new fuzzy tests

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
