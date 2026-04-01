# Specification: SGR Hooks + Plan→Execute Pipeline + Fuzzy Search

**Track ID:** sgr-hooks-pipeline_20260401
**Type:** Feature
**Created:** 2026-04-01
**Status:** Draft

## Summary

Leverage sgr-agent's underused infrastructure to improve Pac1Agent: (1) activate Agent hooks (`prepare_context`, `prepare_tools`, `after_action`) that the loop already calls but Pac1Agent ignores, (2) add a Plan→Execute pipeline where a PlanningAgent first decomposes the task into concrete steps with tool_hints, then the executor follows the plan step-by-step with filtered tools, (3) add strsim Levenshtein fuzzy matching for CRM contact/account name search alongside existing regex fuzzy.

These are universal improvements — no task-specific logic. The hooks move action ledger and security checks into the proper framework extension points. The planning pipeline reduces wasted steps by giving the model a concrete plan before acting. Fuzzy search catches name typos that regex misses.

## Acceptance Criteria

- [ ] `prepare_context()` injects action ledger + CRM state + classification summary into context each step
- [ ] `prepare_tools()` dynamically filters tools based on step count and task_type (replaces inline router logic in decide_stateful)
- [ ] `after_action()` runs post-read injection check and records to action ledger (replaces LoopEvent callback)
- [ ] Plan→Execute pipeline: first run_loop with read-only PlanningAgent (≤5 steps) → extract Plan → second run_loop with Pac1Agent guided by plan
- [ ] Plan steps include tool_hints that filter available tools per step
- [ ] strsim Levenshtein added to SearchTool smart_search as fallback after fuzzy_regex
- [ ] strsim matching in CrmGraph for contact/account name lookup
- [ ] No regression on existing tests (69 tests pass)
- [ ] No task-specific patterns or task IDs in code
- [ ] cargo test + cargo build clean

## Dependencies

- sgr-agent crate (PlanningAgent, Agent hooks, ToolRegistry.filter)
- strsim 0.11 (add as direct dep — sgr-agent has it but doesn't re-export)

## Out of Scope

- SwarmManager / parallel sub-agents (sequential pipeline is simpler, no concurrency issues)
- Compaction (helpful but separate concern)
- Evolution module (needs benchmark infrastructure first)
- Changing sgr-agent core code

## Technical Notes

- Agent hooks (`prepare_context`, `prepare_tools`, `after_action`) are called by `run_loop_interactive()` at lines 328, 330, 482/508/539/575 — already wired, just need Pac1Agent implementations
- PlanningAgent wraps any Agent, enforces read-only tools, sets `plan_mode: true` in context
- Plan struct: `{summary, steps: [{description, files, tool_hints}]}` — stored in `ctx.custom["plan"]`
- `ToolRegistry::filter(names)` creates filtered view — use with tool_hints per step
- strsim::normalized_levenshtein already used in registry.rs for fuzzy tool resolve
- Current action ledger recording happens in LoopEvent callback in main.rs — after_action hook is the proper place
- Current tool routing happens inline in decide_stateful — prepare_tools is the proper place
