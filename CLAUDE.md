# CLAUDE.md — agent-bit (PAC1 Agent)

BitGN PAC1 Challenge agent in Rust, powered by sgr-agent.

## Build & Run

```bash
cargo build
OPENAI_API_KEY=sk-... cargo run -- --list              # list tasks
OPENAI_API_KEY=sk-... cargo run -- --task t16           # single task
OPENAI_API_KEY=sk-... cargo run                         # all 25 tasks
PAC1_DEBUG=1 cargo run -- --task t16                    # verbose PCM responses
```

## Architecture

```
src/main.rs   — CLI, orchestration, agent loop (ToolCallingAgent + run_loop)
src/bitgn.rs  — HarnessService client (Connect-RPC/JSON)
src/pcm.rs    — PcmRuntime client (11 file-system RPCs)
src/tools.rs  — 11 Tool implementations wrapping PcmClient
```

Depends on `sgr-agent` from `../../shared/rust-code/crates/sgr-agent` (path dep).

## Key Design Decisions

- **Connect-RPC via reqwest+serde_json** — no proto codegen, just HTTP POST with JSON
- **ToolCallingAgent** from sgr-agent — native OpenAI function calling
- **Responses API** — with `tool_choice: required` and `function_call_output` format
- **Pre-grounding** — tree + AGENTS.md + context loaded before LLM loop
- **Auto-submit fallback** — if agent doesn't call `answer`, last assistant text is submitted

## CLI Flags

| Flag | Default | Description |
|------|---------|-------------|
| `--benchmark` | `bitgn/pac1-dev` | Benchmark ID |
| `--task` | (all) | Run specific task |
| `--model` | `gpt-5.4-mini` | LLM model |
| `--max-steps` | 30 | Max agent loop steps |
| `--list` | false | List tasks and exit |

## sgr-agent Fixes Applied

Two bugs fixed in oxide_client.rs for Responses API compatibility:
1. `tools_call` uses `build_request_with_tool_outputs` when messages contain tool results
2. `tool_choice: required` to prevent text-only responses that lose content
