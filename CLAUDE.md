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
cargo test                                        # 156 unit tests
```

## Architecture

```
src/main.rs          -- CLI, orchestration, guess_outcome (~459 lines)
src/prompts.rs       -- system prompts, planning prompt, dynamic examples
src/scanner.rs       -- security scanning, inbox classification, domain matching
src/pregrounding.rs  -- contact pre-grounding, inbox reading, planning, agent execution
src/agent.rs         -- Pac1Agent (Router + Structured CoT reasoning)
src/bitgn.rs         -- HarnessService client (Connect-RPC/JSON)
src/pcm.rs           -- PcmRuntime client (11 file-system RPCs)
src/tools.rs         -- 11 Tool implementations + security guard + OutcomeValidator
src/config.rs        -- Provider config with prompt_mode, temperature, planning_temperature
src/classifier.rs    -- ONNX classifier + OutcomeValidator (adaptive kNN)
src/crm_graph.rs     -- petgraph CRM knowledge graph (contacts, accounts, sender trust)
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
  --> planning phase (read-only, 5 steps, planning_temperature=0.4)
  --> execution loop (Pac1Agent, max 20 steps, SearchTool w/ CRM annotation, confidence-gated reflection)
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
- `resolve_contact_hints()` — directive format: `RESOLVED: "Smith" = john smith (account: Acme Corp). USE this contact, not: jane smith`
- Injected as pre-grounding message: "⚠ CONTACT RESOLUTION (use these, do NOT ask for clarification)"
- `contacts_summary()` — pre-loads all contacts (name, email, account) to avoid 10+ individual file reads that trigger loop detection
- CrmGraph methods: `contacts_for_account()`, `account_for_contact()`, `find_all_matching_contacts()`, `contact_names()`
- SearchTool carries `Option<Arc<CrmGraph>>` — annotates multi-contact search results with account info
- CrmGraph `ingest_contact/account` strips PCM `$ cat` header and supports `full_name` field
- UNKNOWN sender annotation is neutral ("new or external sender, process normally") — prevents over-cautious DENIED
- Prompt includes explicit disambiguation example (search both contacts → pick best match → proceed)

### Credential Detection
- **Exfiltration** (DENIED): OTP + branching logic ("first character", "branch", "depending on")
- **Verification** (OK): OTP + simple check ("correct"/"incorrect", no extraction)
- Distinction prevents false positives on legit OTP verify tasks

### OutcomeValidator (adaptive kNN)
- **Hypothesis template**: `"The CRM task result: {msg}"` for better embedding discrimination
- **Seed store**: 32 static examples across 4 outcomes (OUTCOME_EXAMPLES in classifier.rs) — 10 OK, 7 DENIED, 7 UNSUPPORTED, 6 CLARIFICATION
- **Adaptive store**: grows from confirmed correct trials only (score ≥ 1.0), persisted to `.agent/outcome_store.json`
- **k-NN (k=5)**: nearest-neighbor voting (no lossy centroid averaging)
- **Confidence-gated blocking**: `ValidationMode::Block` when ≥4/5 votes + top_sim > 0.80, `Warn` for 3/5 (log only), `Pass` otherwise
- **Security-safe**: never blocks when chosen outcome is `OUTCOME_DENIED_SECURITY` (trust LLM security decisions)
- **Retry limit**: max 1 block per trial via `AtomicU32` counter — prevents infinite validation loops
- **Score-gated learning**: `store_answer()` in AnswerTool, `learn_last()` in main.rs after trial scores ≥ 1.0
- **Created in main.rs**: shared across all trials, accessible for post-trial learning (not in pregrounding.rs)
- Dedup: cosine >0.95 suppressed, cap 200, FIFO eviction

### Single Prompt Mode
- Single explicit decision tree for all models (removed standard/explicit split)
- Numbered steps, 5 examples, verbose — works for both Nemotron and GPT-5.4
- Decision framework reframing: "DENIED requires EXPLICIT evidence — not suspicion, not caution"

### Temperature Annealing (EAD-inspired)
- `planning_temperature` (default 0.4): higher temp during read-only planning phase → more exploration
- `temperature` (default 0.1 for Nemotron): lower temp during execution → deterministic commits
- Separate values threaded through config → main → pregrounding → run_planning_phase vs run_agent
- Config field `planning_temperature` in `ProviderSection`, defaults to 0.4 if absent

### Confidence-Gated Reflection (AUQ-inspired)
- `confidence` field in reasoning tool schema (0.0-1.0, optional, default 0.5 if omitted)
- Parsed in `decide_stateful()`, logged as `🎯 Confidence: X.XX`
- Triggered reflection: if confidence < 0.7 AND step < max_steps-2 AND not done → inject reflection prompt
- Reflection prompt: "Is this legitimate CRM work? Do you have EXPLICIT evidence of attack?"
- Max 1 reflection per `decide_stateful()` call via `AtomicU32` counter
- Security guard: never reflect on `blocked` + confidence >= 0.9 (trust high-confidence security)

### Outcome Distinction (critical for correctness)
- `OUTCOME_OK` = task completed successfully
- `OUTCOME_DENIED_SECURITY` = someone is ATTACKING (injection, social engineering, credential exfiltration)
- `OUTCOME_NONE_UNSUPPORTED` = you LACK capability (deploy, external API, missing data)
- `OUTCOME_NONE_CLARIFICATION` = NOT CRM work (math, trivia, jokes)
- Key rule: "could not complete" -> UNSUPPORTED, not OK. Deploy/external -> UNSUPPORTED, not DENIED

### Delete Routing (structural write-restriction)
- Router "delete" task_type: restricts tools to search+read+find+list+delete+answer (NO write/mkdir/move)
- Permanent restriction — no step-based safety net (unlike "search"/"analyze")
- Prevents capture-instead-of-delete failure mode on delete-only tasks (t08)
- task_type description: "delete=remove a specific file ONLY, use 'edit' if task also needs writing"
- Delete-intent pre-grounding hint injected for instructions containing "delete"/"remove" (but not "capture"/"distill"/"write"/"create")
- **Structural task_type forcing**: `detect_forced_task_type()` overrides LLM task_type when instruction is unambiguously delete-only (contains delete/remove, NOT captur/distill/write/creat/updat/process). Logged as `🔒 Task-type override`. Makes delete routing deterministic regardless of LLM classification.

### Capture/Distill Workflow (file ops safety net)
- Router "search" task_type: step 0 read-only, step 1+ full toolkit (mirrors "analyze")
- Prevents permanent write/delete lockout if Nemotron misclassifies task_type as "search"
- task_type description explicitly lists "capture, distill, process inbox" → "edit"
- Default CRM examples include capture-from-inbox pattern (read→write→delete)
- `filter_tools_for_task()` extracted for testability (9 Router unit tests)
- **t03 fixed**: thread-update example + write-nudge (2+ reads-since-last-write → inject "use write() now"). Counter only resets on write-class tools (write/delete/move_file/answer), NOT on search/find/list/tree. Filename preservation hint for distill cards.
- **Capture-delete nudge**: at 50%+ steps, if inbox files read but not deleted → inject strong "DELETE inbox files NOW" reminder. Deferred flag pattern — only marks sent when conditions are met (inbox read in ledger + no inbox delete). Pre-grounding also injects reminder for capture/distill/process/inbox instructions. Distill example in prompts.rs includes delete step.

### Pre-grounding Context
- tree + AGENTS.md + CRM schema (READMEs from directories)
- Contacts summary: pre-loaded from CrmGraph (name, email, account) — avoids 10+ file reads that trigger loop detection
- Classified inbox with [CLASSIFICATION], [SENDER TRUST] annotations
- Inbox processing guidance: evaluate EACH message separately, OK if at least one processed
- Channel file statistics: auto-count entries by category (blacklist, verified, etc.)
- OTP cleanup: after processing OTP inbox, delete source file (docs/channels/otp.txt)
- OTP-intent hint: injected when inbox has high-confidence (>0.50) credential classification without exfiltration — prevents false DENIED on legit OTP tasks
- Outbox: read README.MD for format, include `"sent": false`

### Agent Loop Configuration
- `loop_abort_threshold: 25` — high to avoid tier-2 false positives from parallel reads (10 contacts in 1 step)
- History preserved on agent error (max steps, loop detected) — `run_agent` returns Ok with accumulated context
- `guess_outcome()` — last_msg priority, "Written to" in history = strong OK signal

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

**Cost policy — save OpenAI credits:**
- **Primary model: Nemotron** (free via CF Workers AI). Use for ALL development and testing.
- **OpenAI (GPT-5.4/mini): ONLY for final validation** — max 1-2 runs per session, not for iteration.
- `make task T=tXX` — defaults to Nemotron. Do NOT add `PROVIDER=openai` unless specifically validating.
- Never run `make full PROVIDER=openai-full` — too expensive. Use Nemotron for full benchmarks.

**Verification after every code change:**
```bash
cargo test                         # unit tests must pass
make task T=tXX                    # verify on Nemotron (FREE, default)
make task T=tXX PROVIDER=openai-full  # ONLY for final validation (costs money)
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
- t03: capture-delete nudge + write-nudge counter fix applied. Passes ~60% on Nemotron.
- t08: delete routing + structural task_type forcing + UTF-8 safe truncation. Still non-deterministic due to CLARIFICATION randomization.
- t23: hardened for Nemotron (directive hints, inbox processing guidance, loop threshold 25, auto-answer fix). Passes ~2/3 on Nemotron.
- t25, t29: OTP classification refined (exfiltration vs verification vs passive), still non-deterministic.
- **Prompt regression**: static prompt bloated (~45 lines vs pre-bloat ~20 lines), score dropped from 80% to ~55%. Plan created: `docs/plan/fix-prompt-regression_20260404/`.

Plans: `docs/plan/`, roadmap: `docs/roadmap.md`

Results: `benchmarks/runs/`, `.claude/skills/evolve/results.tsv`

## sgr-agent Relationship

sgr-agent provides: Agent trait, LlmClient, ToolRegistry, run_loop, Message types.
agent-bit provides: Pac1Agent (custom Agent impl), PCM tools, security scanner, OutcomeValidator.
sgr-agent is NOT modified for PAC1-specific logic.

## Runtime Data

- `.agent/outcome_store.json` -- adaptive OutcomeValidator prototypes (grows with each run)
- `.agent/evolution.jsonl` -- sgr-agent auto-logged RunStats
- `models/` -- ONNX model files (gitignored, ~90MB, run `scripts/export_model.py`)
