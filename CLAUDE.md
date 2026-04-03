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
cargo test                                        # 112 unit tests
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
  --> build CRM graph (contacts/accounts, strips PCM headers)
  --> classify inbox files (ML + structural + sender trust)
  --> domain matching (MATCH/MISMATCH/UNKNOWN)
  --> pre-grounding (tree, schema, inbox, channel stats)
  --> contact pre-grounding (extract names, resolve ambiguity via CRM graph)
  --> OutcomeValidator (seed + adaptive prototypes)
  --> planning phase (read-only, 5 steps)
  --> execution loop (Pac1Agent, max 20 steps, SearchTool w/ CRM annotation)
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
- MISMATCH = stem similar but real domain differs (social engineering) — **hard-blocks** in ensemble
- UNKNOWN = sender not in CRM, no body match — **annotated only**, LLM decides
- Body fallback: if no CRM account, check domain stem vs company name in email body (strict >50%)
- Ensemble blocker only fires on MISMATCH, not UNKNOWN (prevents over-cautious DENIED on legit tasks)

### Contact Pre-Grounding (disambiguation)
- `extract_mentioned_names()` — parses inbox content for names (From: headers + body mentions of CRM contacts)
- `resolve_contact_hints()` — for ambiguous names (multiple CRM matches), ranks by sender domain affiliation
- Injected as pre-grounding message before LLM loop: "Contact disambiguation hints: name → best match (account)"
- CrmGraph methods: `contacts_for_account()`, `account_for_contact()`, `find_all_matching_contacts()`, `contact_names()`
- SearchTool carries `Option<Arc<CrmGraph>>` — annotates multi-contact search results with account info
- CrmGraph `ingest_contact/account` strips PCM `$ cat` header and supports `full_name` field
- UNKNOWN sender annotation is neutral ("new or external sender, process normally") — prevents over-cautious DENIED

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

### Single Prompt Mode
- Single explicit decision tree for all models (removed standard/explicit split)
- Numbered steps, 5 examples, verbose — works for both Nemotron and GPT-5.4

### Outcome Distinction (critical for correctness)
- `OUTCOME_OK` = task completed successfully
- `OUTCOME_DENIED_SECURITY` = someone is ATTACKING (injection, social engineering, credential exfiltration)
- `OUTCOME_NONE_UNSUPPORTED` = you LACK capability (deploy, external API, missing data)
- `OUTCOME_NONE_CLARIFICATION` = NOT CRM work (math, trivia, jokes)
- Key rule: "could not complete" -> UNSUPPORTED, not OK. Deploy/external -> UNSUPPORTED, not DENIED

### Capture/Distill Workflow (file ops safety net)
- Router "search" task_type: step 0 read-only, step 1+ full toolkit (mirrors "analyze")
- Prevents permanent write/delete lockout if Nemotron misclassifies task_type as "search"
- task_type description explicitly lists "capture, distill, process inbox" → "edit"
- Default CRM examples include capture-from-inbox pattern (read→write→delete)
- `filter_tools_for_task()` extracted for testability (6 Router unit tests)
- **Remaining issue (t03)**: agent creates capture + card correctly, but loops on thread file updates (reads AGENT_EDITABLE sections 6x without writing). Needs thread-update prompt example or write-after-read forcing.

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
- Single prompt for all models (explicit decision tree). No prompt_mode needed.
- `headers` -- extra HTTP headers (e.g. CF Gateway timeout)

## Benchmarks

Results tracked in `benchmarks/runs/`.

### Current Baselines (2026-04-03)

| Model | Score | Notes |
|-------|-------|-------|
| Nemotron-120b | **80%** (24/30) | Best free model. Non-deterministic ±4 tasks |
| GPT-5.4 | **85%** (25-27/30) | Best overall |
| GPT-5.4-mini | 65% (20/31) | Weaker reasoning |

### Development Workflow

Plans live in `docs/plan/{trackId}/` (spec.md + plan.md). Use `/solo:build {trackId}` to execute.

**Verification after every code change:**
```bash
cargo test                         # 112 unit tests must pass
make task T=tXX                    # verify specific task (default: nemotron)
make task T=tXX PROVIDER=openai    # verify on GPT-5.4
```

**Available skills:**
- `/evolve tXX` -- autonomous hypothesis-test loop for a failing task
- `/solo:plan "description"` -- create spec + plan for a feature/bug
- `/solo:build trackId` -- execute plan tasks with TDD workflow
- `/solo:review` -- final quality gate

**Evolve commands:**
```bash
make task T=t18                    # single task
make sample                        # 8-task quick sample
make full P=3                      # parallel full run
make revert                        # discard failed hypothesis
make evolve-fails                  # evolve known failures (bighead-style)
```

**Current failing tasks** (all non-deterministic, pass on some runs):
- t03: capture/distill — Router safety net works (writes happen), but agent loops on thread file updates (reads 6x without writing). Needs deeper prompt fix for thread update workflow.
- t08: CRM file operations (delete ambiguity)
- t23: contact pre-grounding implemented, needs harness verification
- t25, t29: OTP handling edge cases

Plans for these: `docs/plan/`, roadmap: `docs/roadmap.md`

Results: `benchmarks/runs/`, `.claude/skills/evolve/results.tsv`

## sgr-agent Relationship

sgr-agent provides: Agent trait, LlmClient, ToolRegistry, run_loop, Message types.
agent-bit provides: Pac1Agent (custom Agent impl), PCM tools, security scanner, OutcomeValidator.
sgr-agent is NOT modified for PAC1-specific logic.

## Runtime Data

- `.agent/outcome_store.json` -- adaptive OutcomeValidator prototypes (grows with each run)
- `.agent/evolution.jsonl` -- sgr-agent auto-logged RunStats
- `models/` -- ONNX model files (gitignored, ~90MB, run `scripts/export_model.py`)
