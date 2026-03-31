# CLAUDE.md — agent-bit (PAC1 Agent)

BitGN PAC1 Challenge agent in Rust, powered by sgr-agent.

## Build & Run

```bash
cargo build
cargo run -- --provider nemotron --list          # list tasks
cargo run -- --provider nemotron --task t16      # single task
cargo run -- --provider nemotron                 # all 26 tasks
cargo run -- --provider nemotron --parallel 3    # parallel execution
cargo run -- --provider nemotron --dry-run       # pre-scan only (no LLM)
cargo test                                        # 36 unit tests
```

## Architecture

```
src/main.rs   — CLI, orchestration, pre-scan security, system prompts
src/agent.rs  — Pac1Agent (Router + Structured CoT reasoning)
src/bitgn.rs  — HarnessService client (Connect-RPC/JSON)
src/pcm.rs    — PcmRuntime client (11 file-system RPCs)
src/tools.rs  — 11 Tool implementations + security guard + search auto-expand
src/config.rs — Provider config with prompt_mode (explicit/standard)
```

Depends on `sgr-agent` from `../../shared/rust-code/crates/sgr-agent` (path dep).

## Key Design Decisions

- **Pac1Agent** — custom Agent impl with 2-phase flow: structured CoT reasoning → routed action
- **Router pattern** — task_type (search/edit/analyze/security) filters available tools per step
- **Structured CoT** — reasoning tool requires task_type, security_assessment, known_facts, plan, done
- **Search auto-expand** — SearchTool auto-reads ≤3 matching files inline (parent document retrieval)
- **Pre-scan security** — rule-based threat_score before LLM (injection/non-CRM detection)
- **Post-read guard** — ReadTool/SearchTool append warnings on suspicious content
- **prompt_mode** — "explicit" (decision tree) for weak models, "standard" for strong models
- **Pre-grounding** — tree + AGENTS.md + inbox files + security hints loaded before LLM loop
- **Auto-submit fallback** — guess_outcome scans full message history

## CLI Flags

| Flag | Default | Description |
|------|---------|-------------|
| `--provider` / `-p` | config llm.provider | Provider from config.toml |
| `--task` | (all) | Run specific task |
| `--max-steps` | config agent.max_steps | Max agent loop steps |
| `--parallel` | 1 | Concurrency limit |
| `--list` | false | List tasks and exit |
| `--dry-run` | false | Pre-scan only, no LLM |
| `--run` | - | Leaderboard run mode |

## Config

Providers in `config.toml`. Key fields per provider:
- `model`, `base_url`, `api_key` / `api_key_env`
- `prompt_mode` — "explicit" (weak models) or "standard" (default)
- `headers` — extra HTTP headers (e.g. CF Gateway timeout)

## sgr-agent Relationship

sgr-agent provides: Agent trait, LlmClient, ToolRegistry, run_loop, Message types.
agent-bit provides: Pac1Agent (custom Agent impl), PCM tools, security scanner.
sgr-agent is NOT modified for PAC1-specific logic.
