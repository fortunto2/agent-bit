# Specification: Agent Boost — 7 Universal Improvement Techniques

**Track ID:** agent-boost-7_20260401
**Type:** Feature
**Created:** 2026-04-01
**Status:** Draft

## Summary

Implement 7 research-backed techniques to improve PAC1 agent accuracy without task-specific hardcoding. Each technique targets a proven failure mode: wrong tool choice, loops, over-caution, missed edge cases, shallow context. All changes are universal — they improve any CRM agent task, not specific task IDs.

Based on research from Anthropic (context engineering), OpenAI (tool-use best practices), LangChain (few-shot), HuggingFace (reflexion), Amazon (agent evaluation), and SGR methodology.

## Acceptance Criteria

- [ ] Few-shot exemplars: 3-4 complete tool-call trajectories in both EXPLICIT and STANDARD prompts
- [ ] SGR pre-grounding: README.md files from CRM directories loaded before agent loop
- [ ] Tool pruning: "analyze" route split into read-phase and write-phase tool sets (max 7 tools per phase)
- [ ] Action ledger: compact history of previous tool calls injected before each decision
- [ ] Classifier ensemble: structural signals (imperatives, system references) combined with ML classifier
- [ ] Reflexion: validation step between reasoning and action phases
- [ ] Adaptive nudge: step-budget warning at >50% steps used without answer
- [ ] Nemotron ≥60% stable (not ±4 variance) on 30 tasks
- [ ] No task-specific patterns or task IDs in code
- [ ] cargo test passes, no regressions

## Dependencies

- semantic-classifier track (completed — ONNX classifier + CRM graph)
- sgr-agent crate (LoopConfig, run_loop, AgentContext)

## Out of Scope

- Changing sgr-agent core loop (use hooks/messages, not crate modifications)
- Training custom models
- Task-specific prompt engineering
- Provider-specific optimizations (must work across Nemotron, GPT, Gemini)

## Technical Notes

- Agent loop: Pac1Agent 2-phase (structured CoT → routed action) in agent.rs
- Router: security/search/edit/analyze routes filter tool availability
- Loop detection: 3-tier in sgr-agent (exact signature, tool frequency, output stagnation)
- Pre-grounding: tree + AGENTS.md + classified inbox + date loaded as messages
- System prompt has `{agents_md}` template — AGENTS.md from PCM injected
- Prompt already says "Read README.md in relevant folders" but agent doesn't pre-load them
- Nemotron uses `prompt_mode = "explicit"` with decision tree
- LoopConfig: max_steps=20, loop_abort_threshold=10
