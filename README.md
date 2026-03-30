# agent-bit

Rust agent for [BitGN PAC1 Challenge](https://bitgn.com) — Personal & Trustworthy autonomous agents benchmark.

Built on [sgr-agent](https://github.com/fortunto2/rust-code) framework with OpenAI Responses API.

## Quick Start

```bash
export OPENAI_API_KEY=sk-...
cargo run -- --list                    # see tasks
cargo run -- --task t16 --max-steps 15 # run one task
cargo run                              # run all 25 tasks
```

## Score

~50-60% on PAC1-DEV with gpt-5.4-mini (25 tasks, randomized per trial).

## Stack

- **Rust** (edition 2024) + tokio async
- **sgr-agent** — LLM client + agent loop with tool calling
- **OpenAI Responses API** — function calling with `tool_choice: required`
- **Connect-RPC** — HTTP/JSON client for BitGN platform API
