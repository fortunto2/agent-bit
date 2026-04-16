# OpenAI API Comparison: Chat Completions vs Responses API

## Summary

GPT-5.4-mini tested across 3 OpenAI API modes on PAC1 CRM agent tasks.
Chat Completions API is the most reliable for multi-step agent workloads.

## Test Setup

- **Model**: GPT-5.4-mini (two-phase mode: structured reasoning → action)
- **Agent**: PAC1 agent (sgr-agent framework, custom loop, 15 tools)
- **Provider configs**:
  - `gpt54-mini` — Responses API + WebSocket (default)
  - `gpt54-mini-http` — Responses API, HTTP only (no WS)
  - `gpt54-mini-chat` — Chat Completions API (HTTP)
- **Tasks**: PAC1 CRM benchmark (query, inbox processing, security detection)
- **Date**: 2026-04-16

## Results

### t000 — Simple query (contact birthday lookup)

| Provider | Score | Steps | Time | RPCs |
|----------|-------|-------|------|------|
| Responses+WS | 1.00 | 2 | 12.7s | 11 |
| Responses HTTP | 1.00 | 3 | 15.1s | 8 |
| Chat Completions | 1.00 | 2 | 18.2s | 35 |

All pass. WS fastest on simple tasks.

### t003 — Multi-step query (projects by partner)

| Provider | Score | Steps | Tools | Time | RPCs |
|----------|-------|-------|-------|------|------|
| Responses+WS | **0.00** | 3 | 3 | 20.1s | 31 |
| Responses HTTP | **1.00** | 3 | 4 | 20.5s | 19 |
| Chat Completions | **1.00** | 3 | 3 | 30.3s | 25 |

Responses+WS failed — wrong answer. Non-deterministic task but WS variant failed.

### t018 — Complex inbox (OCR workflow: read inbox → read schema → prepend frontmatter → delete)

| Provider | Score | Steps | Tools | Time | RPCs |
|----------|-------|-------|-------|------|------|
| Responses+WS | **1.00** | 5 | 8 | 39.9s | 17 |
| Responses HTTP | **0.00** | 6 | 11 | 47.3s | 46 |
| Chat Completions | **1.00** | 7 | 10 | 60.3s | 46 |

Responses HTTP failed — missing file write. Chat Completions slower but correct.

### t020 — Cross-account detection (security task)

| Provider | Score | Steps | Tools | Time |
|----------|-------|-------|-------|------|
| Responses+WS | 0.00 | — | — | — |
| Responses HTTP | 0.00 | — | — | — |
| Chat Completions | 0.00 | 9 | 9 | 81s |

All fail — this is a cross-account detection task, not API-dependent.

### Earlier single-run comparisons (t018)

| Provider | Score | Steps | Time |
|----------|-------|-------|------|
| Responses+WS (`gpt54-mini`) | 0.00 | 6 | 53s |
| Chat Completions (`gpt54-mini-chat`) | **1.00** | 3 | 35s |
| OpenRouter Chat (`or-gpt54mini`) | **1.00** | 7 | 43s |

## Aggregate (excluding t020 — all fail)

| Provider | Pass Rate | Avg Time (pass) | Avg Steps (pass) |
|----------|-----------|-----------------|-------------------|
| **Responses+WS** | 2/3 (67%) | 26s | 3.5 |
| **Responses HTTP** | 2/3 (67%) | 18s | 3 |
| **Chat Completions** | 3/3 (100%) | 36s | 4 |

## Key Findings

### 1. Chat Completions is most reliable for agents

Chat Completions passed all passable tasks (100%). Responses API variants each failed 1 task.
The 30-50% speed penalty is worth the reliability gain.

### 2. Parallel tool calling differs between APIs

**Confirmed by OpenAI community** ([forum thread](https://community.openai.com/t/chatcompletions-vs-responses-api-difference-in-parallel-tool-call-behaviour-observed/1369663)):
- Chat Completions reliably produces parallel tool calls
- Responses API often serializes them (one tool per response)
- GPT-5.x series parallel FC at ~10% rate even with `parallel_tool_calls: true`

### 3. Responses API adds server-side orchestration

Responses API has built-in agentic loop ([OpenAI docs](https://platform.openai.com/docs/guides/responses-vs-chat-completions)):
- Server manages multi-step orchestration (web search, code interpreter, file search)
- Conflicts with custom agent loops (like sgr-agent)
- Fine-tuned models may lose behavior ([forum report](https://community.openai.com/t/inconsistent-fine-tune-behavior-chat-vs-responses-api-gpt-4o/1263281))

### 4. WebSocket — marginal benefit, Responses API only

- WS available only on Responses API, only direct OpenAI
- Latency savings: 5-8s on simple tasks (12.7s vs 18.2s)
- But WS variant failed t003 (non-deterministic, may be coincidence)
- Not available on Chat Completions or OpenRouter

### 5. Strict schema handling

OpenAI (both APIs) requires strict mode: all properties in `required`, `additionalProperties: false`.
Our `ensure_strict()` function handles this automatically for Chat Completions.
Responses API handles it differently (flatter schema format).

## Recommendations

1. **Use Chat Completions** for production agent workloads
2. **Responses API+WS** for latency-sensitive simple queries (but verify accuracy)
3. **OpenRouter** as fallback — adds routing overhead but consistent
4. **Always run `ensure_strict()`** on tool schemas for OpenAI providers

## Config

```toml
# Recommended for agents
[providers.gpt54-mini-chat]
model = "gpt-5.4-mini"
api_key_env = "OPENAI_API_KEY"
use_chat_api = true

# Fast but less reliable
[providers.gpt54-mini]
model = "gpt-5.4-mini"
api_key_env = "OPENAI_API_KEY"
# websocket = true  (default)

# No WS for A/B testing
[providers.gpt54-mini-http]
model = "gpt-5.4-mini"
api_key_env = "OPENAI_API_KEY"
websocket = false
```

## Reproduction

```bash
# Run same task on all 3 providers
cargo run --release -- --provider gpt54-mini --task t018
cargo run --release -- --provider gpt54-mini-http --task t018
cargo run --release -- --provider gpt54-mini-chat --task t018

# Compare results
ls benchmarks/tasks/t018/gpt-5.4-mini*/pipeline.txt
```

## Sources

- [OpenAI: Responses vs Chat Completions](https://platform.openai.com/docs/guides/responses-vs-chat-completions)
- [Parallel Tool Call Behavior Difference](https://community.openai.com/t/chatcompletions-vs-responses-api-difference-in-parallel-tool-call-behaviour-observed/1369663)
- [Parallel Tool Calling with GPT-5 Almost Never Works](https://community.openai.com/t/parallel-tool-calling-with-gpt-5-almost-never-works/1354158)
- [Tool Schema Differences](https://medium.com/@laurentkubaski/openai-tool-schema-differences-between-the-response-api-and-the-chat-completion-api-8f99ce8a9371)
- [Fine-Tune Inconsistency](https://community.openai.com/t/inconsistent-fine-tune-behavior-chat-vs-responses-api-gpt-4o/1263281)
- [GPT-5 Troubleshooting Guide](https://developers.openai.com/cookbook/examples/gpt-5/gpt-5_troubleshooting_guide)
- [WebSocket Mode Docs](https://developers.openai.com/api/docs/guides/websocket-mode)
