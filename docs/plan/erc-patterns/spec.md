# Specification: ERC Patterns for PAC1

**Track ID:** erc-patterns_20260331
**Type:** Feature
**Created:** 2026-03-31
**Status:** Draft

## Summary

Implement 3 patterns from Enterprise RAG Challenge winners to boost PAC1 agent performance on weak models (Nemotron 120B). Key insight: HybridAgent in sgr-agent stays untouched (shared across agents). Instead, create a new `Pac1Agent` in agent-bit that wraps HybridAgent, adding PAC1-specific routing, structured CoT, and smart search.

Architecture: `Pac1Agent` implements `Agent` trait, wraps `HybridAgent<Llm>`, overrides `prepare_context`, `prepare_tools`, and provides a custom reasoning tool schema with task classification + security assessment fields. The search tool auto-expands results with parent document content.

## Acceptance Criteria

- [ ] `Pac1Agent` created in `src/agent.rs`, wrapping HybridAgent with custom reasoning schema
- [ ] Reasoning tool includes: `task_type` (enum), `security_assessment`, `known_facts`, `plan`
- [ ] Task type classification routes to specialized tool subsets via `prepare_tools`
- [ ] SearchTool auto-expands results (≤3 files → inline full file content)
- [ ] HybridAgent in sgr-agent is NOT modified
- [ ] Nemotron scores ≥70% on PAC1-dev (currently ~60%)
- [ ] No regression on gpt-5.4-mini (maintain 68%+)
- [ ] cargo test passes (new + existing tests)

## Dependencies

- sgr-agent `Agent` trait (prepare_context, prepare_tools, after_action hooks)
- sgr-agent `HybridAgent<Llm>` as inner agent
- sgr-agent `AgentContext.custom` HashMap for routing state

## Out of Scope

- Modifying sgr-agent core (HybridAgent, Agent trait, agent_loop)
- Self-consistency / majority voting (too expensive for Nemotron)
- LLM reranking (requires extra LLM call per step)
- Evaluation pipeline automation

## Technical Notes

- `PlanningAgent` in sgr-agent demonstrates the wrapper pattern (implements Agent, delegates decide() to inner)
- `prepare_tools` hook filters tools per step — ideal for Router
- `prepare_context` can set routing decisions in `ctx.custom`
- `after_action` fires after each tool — can track search patterns
- Custom reasoning_tool_def is local to agent implementation, not shared
- SearchTool already has `guard_content()` — auto-expand builds on same pattern
