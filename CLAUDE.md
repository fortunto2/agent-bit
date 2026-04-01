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
cargo test                                        # 69 unit tests
```

## Architecture

```
src/main.rs       — CLI, orchestration, pre-scan security, system prompts
src/agent.rs      — Pac1Agent (Router + Structured CoT reasoning)
src/bitgn.rs      — HarnessService client (Connect-RPC/JSON)
src/pcm.rs        — PcmRuntime client (11 file-system RPCs)
src/tools.rs      — 11 Tool implementations + security guard + search auto-expand
src/config.rs     — Provider config with prompt_mode (explicit/standard)
src/classifier.rs — ONNX embedding classifier (all-MiniLM-L6-v2, cosine similarity)
src/crm_graph.rs  — petgraph CRM knowledge graph (contacts, accounts, sender trust)
scripts/export_model.py — Export ONNX model + tokenizer + class embeddings
models/           — ONNX model files (gitignored, ~90MB, run export_model.py)
```

Depends on `sgr-agent` from `../../shared/rust-code/crates/sgr-agent` (path dep).

## Key Design Decisions

- **Plan→Execute pipeline** — PlanningAgent (≤5 read-only steps) decomposes task into Plan{steps, tool_hints}, injected as system context for main executor
- **Agent hooks** — `after_action()` records action ledger + runs structural injection check on read/search output. `prepare_context()` exposes step_count + ledger in ctx.custom
- **Fuzzy search (strsim)** — Levenshtein distance matching in SearchTool (filename fallback) and CrmGraph (contact name fuzzy-find, sender trust upgrade)
- **Pac1Agent** — custom Agent impl with 3-phase flow: structured CoT reasoning → reflexion → routed action
- **Router pattern** — task_type (search/edit/analyze/security) filters available tools per step
- **Tool pruning** — "analyze" route uses read-only tools on first step, full toolkit after ≥1 read
- **Structured CoT** — reasoning tool requires task_type, security_assessment, known_facts, plan, done
- **Reflexion** — validation step between reasoning and action (standard mode only). Asks model to verify plan before acting. Max 1 reflexion per step.
- **Action ledger** — compact history of previous tool calls (max 10, 80 chars each) injected as context before reasoning. Prevents repeat searches.
- **Adaptive nudge** — one-time "complete now" message injected when step > 50% budget without answer
- **Few-shot trajectories** — 4 tool-call examples in both system prompts (CRM lookup, injection, OTP, non-CRM)
- **SGR pre-grounding** — README.md from tree directories loaded as "CRM Schema" context (max 2000 chars)
- **Search auto-expand** — SearchTool auto-reads ≤3 matching files inline (parent document retrieval)
- **Classifier ensemble** — ONNX ML classifier + structural signal detection (imperatives, system refs, base64, zero-width unicode). Weighted: 0.7*ML + 0.3*structural. ≥2 structural signals boost injection to min 0.5.
- **Instruction classifier** — ML+structural ensemble also runs on task instruction text, blocking injection >0.5 and non_work >0.5 before agent loop
- **CRM knowledge graph** — petgraph builds in-memory graph from PCM contacts/accounts at trial start. Validates sender email domain → SenderTrust (KNOWN/PLAUSIBLE/CROSS_COMPANY/UNKNOWN).
- **Pre-scan security** — minimal threat_score (only HTML injection: `<script>`, `<iframe>`, `javascript:`). All semantic patterns handled by classifier.
- **Post-read guard** — ReadTool/SearchTool append warnings on suspicious content
- **prompt_mode** — "explicit" (decision tree, no reflexion) for weak models, "standard" (with reflexion) for strong models
- **Pre-grounding** — tree + AGENTS.md + CRM schema + classified inbox files + classification summary loaded before LLM loop
- **Loop threshold** — abort after 6 repeated actions (down from 10, PAC1 tasks are short)
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

## Benchmarks

Results tracked in `benchmarks/runs/`. After each significant change, run benchmark and log results.

### Current Baselines (2026-03-31)

| Model | Commit | Score | Notes |
|-------|--------|-------|-------|
| gpt-5.4 | 05a4aed | **71.4%** (20/28) | +5 fixed from 64% baseline (t04,t05,t08,t14,t23) |
| gpt-5.4-mini | 05a4aed | 100% (8/8 sample) | |
| nemotron-120b | 40410a3 | 60% (18/30 full), 87.5% (7/8 sample) | Non-deterministic: ±4 tasks between runs |

### Known Unsolved Tasks

- **t18, t20, t22** — subtle inbox traps that bypass pre-scan AND model misses
- **t24** — legit task where model is over-cautious
- **t25** — injection caught as CLARIFICATION instead of DENIED (severity mismatch)
- **t19, t28** — new tasks, need investigation

### How to benchmark

```bash
# 8-task quick sample
for t in t01 t02 t03 t04 t05 t09 t16 t21; do cargo run -- --provider openai --task $t &>/tmp/oai-$t.log & done; wait
# Full 26 tasks
cargo run -- --provider openai --parallel 3
```

Log results to `benchmarks/runs/{date}__{provider}__{commit}.md`.

## sgr-agent Relationship

sgr-agent provides: Agent trait, LlmClient, ToolRegistry, run_loop, Message types.
agent-bit provides: Pac1Agent (custom Agent impl), PCM tools, security scanner.
sgr-agent is NOT modified for PAC1-specific logic.
