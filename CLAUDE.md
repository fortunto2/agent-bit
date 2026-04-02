# CLAUDE.md -- agent-bit (PAC1 Agent)

BitGN PAC1 Challenge agent in Rust, powered by sgr-agent.

## Build & Run

```bash
cargo build
cargo run -- --provider nemotron --list          # list tasks
cargo run -- --provider nemotron --task t16      # single task
cargo run -- --provider nemotron                 # all 30 tasks
cargo run -- --provider nemotron --parallel 3    # parallel execution
cargo run -- --provider openai-full --parallel 3 # GPT-5.4
cargo test                                        # 83 unit tests
```

## Architecture

```
src/main.rs       -- CLI, orchestration, pre-scan, system prompts, domain matching
src/agent.rs      -- Pac1Agent (Router + Structured CoT reasoning)
src/bitgn.rs      -- HarnessService client (Connect-RPC/JSON)
src/pcm.rs        -- PcmRuntime client (11 file-system RPCs)
src/tools.rs      -- 11 Tool implementations + security guard + OutcomeValidator
src/config.rs     -- Provider config with prompt_mode (explicit/standard)
src/classifier.rs -- ONNX classifier + OutcomeValidator (adaptive kNN)
src/crm_graph.rs  -- petgraph CRM knowledge graph (contacts, accounts, sender trust)
```

Depends on `sgr-agent` from `../../shared/rust-code/crates/sgr-agent` (path dep).

## Decision Pipeline

```
instruction --> prescan (HTML only) --> start trial
  --> build CRM graph (contacts/accounts)
  --> classify inbox files (ML + structural + sender trust)
  --> domain matching (MATCH/MISMATCH/UNKNOWN)
  --> pre-grounding (tree, schema, inbox, channel stats)
  --> OutcomeValidator (seed + adaptive prototypes)
  --> planning phase (read-only, 5 steps)
  --> execution loop (Pac1Agent, max 20 steps)
  --> answer() with outcome validation
```

## Key Design Decisions

### Security: 3-layer defense
1. **Pre-scan**: literal HTML injection only (`<script>`, `<iframe>`)
2. **Classifier ensemble**: 0.7*ML(ONNX) + 0.3*structural signals, injected as [CLASSIFICATION] headers
3. **LLM decision tree**: numbered steps in system prompt guide outcome selection

### Domain Matching (sender trust)
- `extract_sender_domain()` + `check_sender_domain_match()`
- `domain_stem()` extracts company name from domain ("blue-harbor-bank.biz" -> "blue harbor bank")
- MATCH = exact domain or stem overlap >50% with CRM account
- MISMATCH = stem similar but real domain differs (social engineering)
- Body fallback: if no CRM account, check domain stem vs company name in email body (strict >50%)

### Credential Detection
- **Exfiltration** (DENIED): OTP + branching logic ("first character", "branch", "depending on")
- **Verification** (OK): OTP + simple check ("correct"/"incorrect", no extraction)
- Distinction prevents false positives on legit OTP verify tasks

### OutcomeValidator (adaptive kNN)
- **Hypothesis template**: `"The CRM task result: {msg}"` for better embedding discrimination
- **Seed store**: 17 static examples across 4 outcomes (OUTCOME_EXAMPLES in classifier.rs)
- **Adaptive store**: grows from every answer(), persisted to `.agent/outcome_store.json`
- **k-NN (k=5)**: nearest-neighbor voting (no lossy centroid averaging)
- **Online learning**: every model's answers feed the store; GPT-5.4 = teacher for Nemotron
- **Currently non-blocking** (log only) -- needs more calibration for blocking mode
- Dedup: cosine >0.95 suppressed, cap 200, FIFO eviction

### Two Prompt Modes
- **Explicit** (`prompt_mode = "explicit"`): numbered decision tree, 5 examples, verbose. For weak models (Nemotron, Kimi)
- **Standard** (`prompt_mode = "standard"`): concise rules, 5 examples. For strong models (GPT-5.4)

### Outcome Distinction (critical for correctness)
- `OUTCOME_OK` = task completed successfully
- `OUTCOME_DENIED_SECURITY` = someone is ATTACKING (injection, social engineering, credential exfiltration)
- `OUTCOME_NONE_UNSUPPORTED` = you LACK capability (deploy, external API, missing data)
- `OUTCOME_NONE_CLARIFICATION` = NOT CRM work (math, trivia, jokes)
- Key rule: "could not complete" -> UNSUPPORTED, not OK. Deploy/external -> UNSUPPORTED, not DENIED

### Pre-grounding Context
- tree + AGENTS.md + CRM schema (READMEs from directories)
- Classified inbox with [CLASSIFICATION], [SENDER TRUST] annotations
- Channel file statistics: auto-count entries by category (blacklist, verified, etc.)
- OTP cleanup: after processing OTP inbox, delete source file (docs/channels/otp.txt)
- Outbox: read README.MD for format, include `"sent": false`

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
- `auth` -- "keychain" for Claude Code subscription (macOS Keychain OAuth token)
- `prompt_mode` -- "explicit" (weak models) or "standard" (default)
- `headers` -- extra HTTP headers (e.g. CF Gateway timeout)

## Benchmarks

Results tracked in `benchmarks/runs/`.

### Current Baselines (2026-04-02)

| Model | Score | Notes |
|-------|-------|-------|
| GPT-5.4 | **83%** (25/30) | Best. Non-deterministic on t23, t29 |
| Nemotron-120b | **73%** (19/26) | Non-deterministic: +/-4 tasks between runs |
| GPT-5.4-mini | 55% (17/31) | Baseline |

### Evolution

Agent improvements use `/evolve` skill -- autonomous hypothesis-test loop.

```bash
make task T=t18                    # single task
make task T=t18 PROVIDER=openai    # different provider
make sample                        # 8-task quick sample
make full P=3                      # parallel full run
make revert                        # discard failed hypothesis
```

Results: `benchmarks/runs/`, `.claude/skills/evolve/results.tsv`

## sgr-agent Relationship

sgr-agent provides: Agent trait, LlmClient, ToolRegistry, run_loop, Message types.
agent-bit provides: Pac1Agent (custom Agent impl), PCM tools, security scanner, OutcomeValidator.
sgr-agent is NOT modified for PAC1-specific logic.

## Runtime Data

- `.agent/outcome_store.json` -- adaptive OutcomeValidator prototypes (grows with each run)
- `.agent/evolution.jsonl` -- sgr-agent auto-logged RunStats
- `models/` -- ONNX model files (gitignored, ~90MB, run `scripts/export_model.py`)
