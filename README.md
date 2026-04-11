# agent-bit

Rust agent for [BitGN PAC1 Challenge](https://bitgn.com) — Personal & Trustworthy autonomous agents benchmark.

Built on [sgr-agent](https://github.com/fortunto2/rust-code) framework.

## Score

| Benchmark | Model | Score | Notes |
|-----------|-------|-------|-------|
| pac1-dev (43 tasks) | Nemotron-120B (CF) | **95.3%** (41/43) | FREE model, primary |
| pac1-dev (43 tasks) | Seed-2.0-pro | **90.7%** (39/43) | Best paid alternative |
| pac1-dev (43 tasks) | GPT-5.4 | **95.3%** (41/43) | Expensive, final validation |
| pac1-prod (104 tasks) | GPT-5.4 | **17%→55%+** | Blind mode, iterating |

30+ models tested across 6 providers (DeepInfra, CF Workers AI, Cerebras, OpenRouter, Modal, OpenAI).

## Quick Start

```bash
cp .env.example .env  # add your API keys
cargo build --release
cargo run --release -- --provider nemotron --list           # list tasks
cargo run --release -- --provider nemotron --task t16       # single task
cargo run --release -- --provider nemotron --parallel 5     # parallel execution
cargo test                                                   # unit tests

# Analysis
make failures M=nemotron     # show failures from dump dirs
make compare                 # side-by-side model comparison
make ai-notes                # list all AI-NOTEs in codebase
cargo run --bin pac1-dash    # TUI dashboard
```

## Architecture

```
Instruction
  → prescan (HTML injection only)
  → Pipeline SM: New → Classified → InboxScanned → SecurityChecked → Ready
  → CRM Graph (petgraph + ONNX embeddings from 10_entities/ or contacts/)
  → Inbox Classifier Ensemble (ML + NLI + structural + sender trust)
  → Feature Matrix (12 features × sigmoid → threat probability)
  → Skill Selection (13 SKILL.md files, hot-reloadable)
  → Agent Loop (Pac1Agent: Structured CoT → Reflexion → Router → Tools)
  → Workflow SM: Reading → Acting → Cleanup → Done
  → Verifier (3-vote self-consistency)
  → answer() → OUTCOME_OK / DENIED / UNSUPPORTED / CLARIFICATION
```

### Key Components

| Component | File | Purpose |
|-----------|------|---------|
| Pipeline SM | `src/pipeline.rs` | Pre-LLM classification, security signals |
| Agent | `src/agent.rs` | Two-phase FC: reasoning → action, router, reflexion |
| Skills | `skills/*.md` | 14 domain skills, hot-reload without rebuild |
| System Prompt | `prompts/system.md` | Hot-reload, evolved via ShinkaEvolve |
| CRM Graph | `src/crm_graph.rs` | Entity graph, ONNX embeddings, cross-account detection |
| Classifier | `src/classifier.rs` | ONNX MiniLM-L6 + NLI DeBERTa + adaptive kNN |
| Feature Matrix | `src/feature_matrix.rs` | 12-feature sigmoid scoring |
| Scanner | `src/scanner.rs` | Security signals, domain matching, sender trust |
| Policy | `src/policy.rs` | File protection, channel trust |
| Workflow SM | `src/workflow.rs` | Runtime guards: budget, write limits, delete control |
| Hooks | `src/hooks.rs` | Data-driven tool completion hooks from AGENTS.MD |
| Tools | `src/tools.rs` | 13 tools + JSON auto-repair (llm_json fork) |
| Dashboard | `src/dashboard.rs` | TUI: model columns, history panel, full diagnostics |

### Hot-Reload (zero rebuild)

- `prompts/system.md` — system prompt
- `skills/*.md` — 14 domain-specific skills
- `config.toml` — temperatures, providers, parallelism

### Ensemble Fallback

```toml
[agent]
fallback_providers = ["seed2", "openai-full"]
```

Primary model fails → verifier detects → retry on fallback model automatically.

## Providers

```bash
cargo run --release -- --provider nemotron    # CF Workers AI (FREE)
cargo run --release -- --provider seed2       # DeepInfra Seed-2.0-pro
cargo run --release -- --provider openai-full # GPT-5.4
cargo run --release -- --provider kimi-k2     # DeepInfra Kimi-K2
```

FC probe at agent start validates model compatibility. 30+ models tested, 4 finalists.

## Leaderboard

```bash
# Submit to leaderboard (parallel)
BENCHMARK=bitgn/pac1-prod LEADERBOARD_PARALLEL=20 \
  cargo run --release -- --provider openai-full --run "my-run-name"

# Force-submit partial run
cargo run --release -- --submit-run "run-XXXXX"
```

## Stack

- **Rust** (edition 2024) + tokio async
- **sgr-agent** — LLM client, agent loop, tool calling
- **ort** (ONNX Runtime) — ML classifier + embeddings
- **petgraph** — entity knowledge graph
- **llm_json** (fork) — LLM JSON auto-repair
- **strsim** — fuzzy string matching
- **ratatui** — TUI dashboard
- **Connect-RPC** — BitGN platform API

## License

MIT
