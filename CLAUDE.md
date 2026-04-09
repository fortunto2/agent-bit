# CLAUDE.md -- agent-bit (PAC1 Agent)

BitGN PAC1 Challenge agent in Rust, powered by sgr-agent.

## Build & Run

```bash
cargo build
cargo run -- --provider nemotron --list          # list tasks WITH hints
cargo run -- --provider nemotron --task t16      # single task
cargo run -- --provider nemotron                 # all 40 tasks
cargo run -- --provider nemotron --parallel 3    # parallel execution
cargo run -- --provider openai-full --parallel 3 # GPT-5.4
cargo test                                        # 240 unit tests
cargo run -- --audit-store                        # audit adaptive store
```

## Architecture

```
src/pipeline.rs      -- enum state machine (New→Classified→InboxScanned→SecurityChecked→Ready)
src/main.rs          -- CLI, orchestration, verify_and_submit, guess_outcome
src/prompts.rs       -- system prompts (V2 annotation-driven + explicit decision tree)
src/skills.rs        -- skill system: loads SKILL.md files, push-model selection via classifier
src/scanner.rs       -- security scanning, inbox classification, domain matching
src/pregrounding.rs  -- context assembly, planning, hints, agent execution (uses pipeline states)
src/agent.rs         -- Pac1Agent (Router + Structured CoT reasoning, two-phase FC)
src/pac1_sgr.rs      -- Pac1SgrAgent (pure SGR mode, single LLM call per step, experimental)
src/bitgn.rs         -- HarnessService client (Connect-RPC/JSON)
src/pcm.rs           -- PcmRuntime client (11 file-system RPCs + read cache + ProposedAnswer)
src/tools.rs         -- 13 Tool implementations + security guard + OutcomeValidator + skill introspection
src/hooks.rs         -- HookRegistry: data-driven tool completion hooks from AGENTS.MD
src/policy.rs        -- File access policy: structural guards for protected paths
src/config.rs        -- Provider config with prompt_mode, temperature, sgr_mode
src/classifier.rs    -- ONNX classifier (security + intent) + NliClassifier (NLI zero-shot) + OutcomeValidator (adaptive kNN)
src/crm_graph.rs     -- petgraph CRM knowledge graph (contacts, accounts, sender trust, ONNX embeddings)
src/feature_matrix.rs -- 12-feature inbox scoring: sigmoid(features × weights), ridge regression calibration
src/dashboard.rs     -- TUI dashboard (ratatui): heatmap + log viewer (cargo run --bin pac1-dash)
```

Depends on `sgr-agent` from `../../shared/rust-code/crates/sgr-agent` (path dep).

### Tool Completion Hooks (src/hooks.rs)

Data-driven workflow guidance. Parsed from AGENTS.MD at trial start:
- `HookRegistry::from_agents_md(content)` extracts hooks from natural language rules
- Pattern: "When adding to {path}, also {action}" → ToolHook {write, path, message}
- Pattern: "Keep files in {path} immutable" → ToolHook {write, path, warning}
- `check(tool_name, path) → Vec<String>` — returns messages to inject

Delivery points (both coexist):
1. **WriteTool**: augments own output (immediate, same LLM call)
2. **Pac1SgrAgent::after_execute**: injects session message (next LLM call)
3. **sgr-agent::app_loop**: `after_execute` hook on SgrAgent trait (framework-level)

### Policy (src/policy.rs) — Single Source of Truth for Authorization

All authorization decisions in ONE module. Other modules delegate here.

**File protection:**
- `check_write(path)` → blocks write/delete to protected files (PcmClient enforces)
- `is_ephemeral(path)` → cleanup files exempt from workflow delete guard (otp.txt)
- `scan_content(text)` → pipeline detects inbox targeting system files (Signal 6)
- Constants: `PROTECTED_BASENAMES`, `POLICY_DIRS`, `EPHEMERAL`

**Channel authorization:**
- `ChannelTrust` — registry of handle → trust level (admin/valid/blacklist/unknown)
- `ChannelTrust::ingest(content)` — parse "handle - level" from channel files
- `ChannelTrust::check(handle)` → ChannelLevel enum
- `ChannelTrust::is_admin(handle)` → only admin can do OTP verification
- Built once per trial in pregrounding, used for inbox annotations

### Workflow State Machine (src/workflow.rs) — Runtime Phase Tracking

Replaces 5 scattered guards with one SM. Tracks agent progress during execution.

**Phases:** `Reading → Acting → Cleanup → Done`
- `advance_step()` → budget/write/capture-delete nudges
- `pre_action(tool, path)` → Block/Warn/Allow (policy + capture guard + delete guard)
- `post_action(tool, path)` → phase transitions + hook messages
- `verification_only` flag → ZERO file changes (OTP oracle)
- `allows_delete` → instruction must mention delete/remove/discard/capture

**Key rule: Block > Warn** — Nemotron ignores warnings, obeys blocks.

### Skills System (src/skills.rs + skills/) — Domain-Specific Prompt Injection

Replaces hardcoded `examples_for_class()` with file-based SKILL.md files. Uses `sgr_agent::skills` (shared crate).

**Push model:** classifier label + intent → skill selection → inject into `{examples}` placeholder.
**Hybrid fallback:** agent can call `list_skills` / `get_skill` tools mid-task to switch workflows.

**Directory:** `skills/` (13 skills, hot-reloadable — edit .md, no rebuild needed)
```
skills/
├── crm-default/SKILL.md       — general CRM (email, contacts, cross-account, multi-inbox)
├── crm-lookup/SKILL.md        — data queries, counting, captured article lookup
├── crm-invoice/SKILL.md       — resend/forward invoices with attachments
├── inbox-processing/SKILL.md  — multi-inbox workflow, channel priority, seq.json
├── capture-distill/SKILL.md   — capture → card → thread → delete source
├── cleanup/SKILL.md           — delete cards/threads (search → read → delete only)
├── security-injection/SKILL.md — injection/social engineering → DENIED
├── security-credential/SKILL.md — OTP workflow (7 examples, exfiltration anti-pattern)
├── non-work/SKILL.md          — non-CRM → CLARIFICATION
├── unsupported/SKILL.md       — external API/deploy/calendar → UNSUPPORTED
├── followup-reschedule/SKILL.md — reschedule dates in accounts + reminders
├── invoice-creation/SKILL.md  — create typed invoice JSON
└── purchase-ops/SKILL.md      — fix purchase ID prefix regression
```

**SKILL.md format** (YAML frontmatter + markdown body):
```yaml
---
name: crm-invoice
description: Resend or forward invoices — MUST include attachments
triggers: [intent_inbox, intent_email]    # classifier labels that activate this skill
priority: 20                               # higher wins when multiple match
keywords: [invoice, resend, forward, INV-] # disambiguate within same trigger group
---
WORKFLOW:
  1. Read inbox... 2. Search invoice... 3. Write outbox WITH attachments...
```

**Selection logic** (`skills::select_body`):
1. Match triggers against [security_label, intent] → candidates
2. If multiple → prefer keyword match in instruction text
3. Highest priority wins
4. Fallback: crm-default

**Self-correcting classification** (agent tools):
- `list_skills` — lists all 13 skills with descriptions
- `get_skill(name)` — loads full skill instructions mid-task
- Both from `sgr_agent::{ListSkillsTool, GetSkillTool}`

**Retry on empty:** If LLM returns text without tool calls, nudge and retry up to 2x.

**When to edit skills vs other components:**
- Wrong workflow/examples → edit `skills/{name}/SKILL.md` (no rebuild needed)
- Wrong skill selected → adjust `triggers` or `keywords` in frontmatter
- Need new skill → create `skills/{name}/SKILL.md` + add to `COMPILED_SKILLS` in `src/skills.rs`

### Architecture Decision Guide

Before ANY fix, check these in order:
1. **policy.rs** — authorization/protection? → Add to policy (`is_word_match` for path guards)
2. **hooks.rs** — "what next" guidance? → Add a hook
3. **workflow.rs** — "when allowed" guard? → Add phase/guard (outbox limit, delete control)
4. **feature_matrix.rs** — scoring/ranking decision? → Adjust weights or add feature
5. **crm_graph.rs** — sender/contact/account trust? → Use graph + ONNX embeddings
6. **pipeline.rs** — pre-LLM classification or pre-execution? → Add signal or pre-execute step
7. **classifier.rs** — content classification? → Use ONNX (retrain via scripts/export_model.py)
8. **skills/** — LLM workflow guidance? → Edit skill .md file (hot-reload, no rebuild)
9. **prompts.rs** — system prompt / decision tree → LAST resort

**Step 7 checklist**: when fixing LLM behavior, FIRST check if the right skill is selected (grep `🎯 Skill:` in logs). If wrong skill → adjust triggers/keywords. If right skill but wrong behavior → edit the skill's SKILL.md file. Adding a rule to a skill is less invasive than editing the system prompt decision tree.

### Feature Matrix (src/feature_matrix.rs) — Batch Inbox Scoring

12-feature vector per inbox message → sigmoid scoring → threat probability.
Inspired by video-analyzer FeatureMatrix pattern.

**Features:** ml_confidence, structural_score, sender_trust, domain_match, has_otp, has_url,
word_count_norm, sentence_length, cross_account_sim, nli_injection, nli_credential, channel_trust.

**Scoring:** `sigmoid(features × weights + bias)` → P(threat) ∈ (0,1).
- `threat_weights()` — hand-tuned preset, validated by ridge regression (R²=0.999)
- `cross_account_weights()` — cross-account detection preset
- `calibrate_ridge()` — learn optimal weights from labeled data (Gauss-Seidel solver)

**Pipeline integration:**
- Computed after inbox scan, before agent execution
- Threat scores injected in `[CLASSIFICATION]` header: `threat: HIGH (75%)`
- **Decision gate:** `sigmoid < 0.5` = safe (P(safe) > P(threat)) — used for capture pre-execute
- Correlation matrix available for feature importance analysis

**CRM Graph embeddings** (crm_graph.rs):
- `compute_account_embeddings()` — L2-normalized MiniLM per account signature
- `similarity_scores(query)` — batch cosine similarity via dot product
- `detect_cross_account()` — comparative: other_sim > sender_sim + 0.1 gap
- `is_word_match()` — path-boundary protected file matcher (no substring false positives)

### Prompt Modes (src/prompts.rs)

Two prompt modes, switchable per provider via `prompt_mode` in config.toml:
- **`"v2"`** (default for Nemotron): annotation-driven, no decision tree. Pipeline annotations = law.
- **`"explicit"`** (default for GPT-5.4): numbered decision tree, more flexible.
V2 outperforms explicit on Nemotron (82.5% vs 75%) because model can't ignore annotations.

### Read Cache (src/pcm.rs)

In-memory cache in PcmClient — shared across all tools via Arc:
- `read()` caches full-file reads by normalized path
- `write()`/`delete()` invalidate cache for same path
- Per-trial lifetime — no stale data between trials

## Key Crates — USE THESE, don't reinvent

| Crate | What | Where used | Use for |
|-------|------|-----------|---------|
| `strsim` | Levenshtein, Jaro-Winkler, normalized similarity | crm_graph (contact fuzzy match), scanner (domain lookalike) | **Any name/string comparison** — never use manual word overlap or `contains()` for fuzzy matching |
| `mailparse` | RFC 5322 email parsing (From/To headers, display names) | scanner (extract_sender_domain), pregrounding (contact names) | **Any email header parsing** — never regex From: headers manually |
| `ort` + `tokenizers` | ONNX inference + HuggingFace tokenizer | classifier.rs (bi-encoder, kNN) | ML classification, embeddings |
| `petgraph` | Directed graph | crm_graph.rs (contacts↔accounts knowledge graph) | CRM relationship queries |
| `ammonia` | HTML sanitization | scanner.rs (prescan) | Safe HTML handling |
| `regex` | Pattern matching | tools.rs (fuzzy search), scanner.rs | Structured pattern extraction |
| `schemars` | JSON Schema from Rust structs | tools.rs (tool parameter schemas) | Tool argument validation |

**Anti-pattern: do NOT use `contains()` / `split_whitespace()` / manual word overlap for string similarity. Use `strsim::normalized_levenshtein()` instead.**

## Decision Pipeline (enum state machine)

```
pipeline::New(instruction)
  → classify()        [STAGE:classify]     — prescan + truncation(tokenizer ##) + ML security + ML intent
pipeline::Classified { instruction, intent, label }
  → scan_inbox()      [STAGE:scan_inbox]   — read inbox files, assess sender trust + security per file
pipeline::InboxScanned { ..., inbox_files, crm_graph }
  → check_security()  [STAGE:security]     — evaluate all inbox assessments, block on first threat
pipeline::SecurityChecked { ... }
  → ready()           [STAGE:ready]        — mark ready for LLM
pipeline::Ready { instruction, intent, inbox_files, crm_graph }
  → [pregrounding.rs] — context assembly, planning, hints, sgr_agent::run_loop()
  → verify_and_submit() — outcome verifier + final RPC
```

Each transition returns `Result<NextState, BlockReason>`. First block short-circuits — LLM never runs.
Stage-by-stage trace in stderr: `[STAGE:classify]`, `[STAGE:scan_inbox]`, `[STAGE:security]`, `[STAGE:ready]`.

Key file: `src/pipeline.rs` — states, transitions, assess_sender(), assess_security().

## Key Design Decisions

### Security: 3-layer defense + 5-signal assess_security
1. **Pre-scan**: literal HTML injection only (`<script>`, `<iframe>`)
2. **Classifier ensemble**: 3-way when NLI available (0.5*ML + 0.3*NLI + 0.2*structural), 2-way fallback (0.7*ML + 0.3*structural). Injected as [CLASSIFICATION] headers with scanner recommendation text.
3. **LLM decision tree**: numbered steps in system prompt guide outcome selection
- **Signal 5** (lookalike guard): requires `CrossCompany + domain_match == "mismatch" + financial` — NOT all CrossCompany senders
- **Recommendation threading**: `SecurityAssessment.recommendation` carries scanner's nuanced guidance (e.g. "OTP verification — process normally") to agent annotations
- **Mismatch warning**: `[⚠ SENDER DOMAIN MISMATCH]` injected when domain_match == "mismatch", matching system prompt step 3

### NLI Zero-Shot Classifier
- **Model**: cross-encoder/nli-deberta-v3-xsmall (22M params, ~273MB ONNX)
- **Export**: `uv run --with transformers --with onnxruntime --with onnx --with onnxscript --with torch --with sentencepiece --with protobuf scripts/export_nli_model.py`
- **Files**: `models/nli_model.onnx`, `models/nli_tokenizer.json`, `models/nli_config.json` (gitignored)
- **Method**: For each (text, hypothesis) pair, computes P(entailment) via softmax over [contradiction, neutral, entailment] logits
- **Hypotheses (v2)**: tuned for CRM discrimination (0.778 entailment) and credential detection (0.636)
- **Ensemble integration**: NLI overrides ML when NLI confidence > 0.5 and labels disagree
- **Limitation**: Low signal on structured messages (OTP, headers) — works best on natural language text
- **Graceful degradation**: If NLI model not present, falls back to 2-way ensemble (no hard dependency)

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

### Account Pre-Grounding (paraphrase resolution)
- `accounts_summary()` — pre-loads all accounts (name, domain, account_manager, contacts) into agent context
- Enables LLM to resolve account paraphrases ("Dutch banking customer" → "Blue Harbor Bank") from pre-loaded data
- `account_manager` field parsed from account files (JSON `account_manager`/`accountManager`, markdown `account_manager:`)
- `annotate_account_results()` — multi-account search results annotated with linked contacts (mirrors `annotate_contact_results()`)
- `expand_query()` — 2-word queries now try reversed word order ("Blom Frederike" → also "Frederike Blom")
- Auto-refs: always merges recent reads with LLM-provided refs (accounts, contacts, invoices)
- Auto-refs: infers account file from contact file (contacts/cont_009.json → accounts/acct_009.json)

### Credential Detection
- **Exfiltration** (DENIED): OTP + branching logic ("first character", "branch", "depending on")
- **Verification** (OK): OTP + simple check ("correct"/"incorrect", no extraction)
- Distinction prevents false positives on legit OTP verify tasks

### OutcomeValidator (adaptive kNN)
- **Hypothesis template**: `"The CRM task result: {msg}"` for better embedding discrimination
- **Seed store**: 65 static examples across 4 outcomes (OUTCOME_EXAMPLES in classifier.rs) — 26 OK, 13 DENIED, 11 UNSUPPORTED, 15 CLARIFICATION
- **Adaptive store**: grows from confirmed correct trials only (score ≥ 1.0), persisted to `.agent/outcome_store.json`
- **k-NN (k=5)**: nearest-neighbor voting (no lossy centroid averaging)
- **Confidence-gated blocking**: `ValidationMode::Block` when ≥4/5 votes + top_sim > 0.80, `Warn` for 3/5 (log only), `Pass` otherwise
- **Security-safe**: never blocks when chosen outcome is `OUTCOME_DENIED_SECURITY` (trust LLM security decisions)
- **Retry limit**: max 1 block per trial via `AtomicU32` counter — prevents infinite validation loops
- **Score-gated learning**: `store_answer()` in AnswerTool, `learn_last()` in main.rs after trial scores ≥ 1.0
- **Created in main.rs**: shared across all trials, accessible for post-trial learning (not in pregrounding.rs)
- Dedup: cosine >0.95 suppressed, cap 200, FIFO eviction

### Outcome Verifier (post-execution)
- **Deferred answer pattern**: AnswerTool stores `ProposedAnswer` via `pcm.propose_answer()` instead of submitting RPC immediately
- After execution loop, `verify_and_submit()` calls `run_outcome_verifier()` — single LLM call with focused 4-way classification
- Verifier prompt (`VERIFIER_PROMPT` in prompts.rs) is much simpler than SYSTEM_PROMPT_EXPLICIT — just validates the outcome code
- Uses function calling schema (`verify_outcome`) returning `{outcome, reason, confidence}`
- **Override policy** (`apply_override_policy()`): **selective security override (v0.4)** — verifier overrides agent ONLY when it detects DENIED_SECURITY with ≥0.95 confidence and agent said OK. Non-security disagreements remain warn-only (6:1 wrong:correct ratio). Agent's own DENIED_SECURITY is never overridden.
- Falls back to proposed answer on verifier LLM error
- When no proposed answer (agent didn't call answer()): uses `guess_outcome()` heuristic directly (verifier confused by CRM content)
- Execution summary: `build_execution_summary()` extracts last 15 relevant tool lines from history, **filters out** security annotations (Security threat, OUTCOME_DENIED, injection, exfiltration) to prevent verifier meta-injection
- Logging: `🔍 Verifier: agree|disagree (conf=X.XX) — reason`
- Key files: `pcm.rs` (ProposedAnswer), `prompts.rs` (VERIFIER_PROMPT), `pregrounding.rs` (run_outcome_verifier), `main.rs` (verify_and_submit)

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

### ML Intent Classification (replaces substring heuristics)
- `classify_intent()` in classifier.rs — 5 intent classes: `intent_delete`, `intent_edit`, `intent_query`, `intent_inbox`, `intent_email`
- Pre-computed centroids in `models/class_embeddings.json` (same MiniLM-L6 ONNX model, separate from security classes)
- `classify()` returns security labels only; `classify_intent()` returns intent labels only — no contamination
- Logged as `Instruction intent: intent_X (confidence)`
- **Task-type forcing**: `detect_forced_task_type()` maps `intent_delete` → `"delete"` task_type override (logged as `🔒 Task-type override`)
- **Skip planning**: `intent_query` skips planning phase entirely — planner hallucinates wrong contacts on simple lookups (t16, t34)
- **Intent-based hints**: `intent_delete` → delete-only hint, `intent_inbox` → capture-delete workflow hint, `intent_query` → include file refs hint
- To add new intents: edit `INTENT_CLASSES` in `scripts/export_model.py`, run `uv run ... scripts/export_model.py` to regenerate centroids

### Delete Routing (structural write-restriction)
- Router "delete" task_type: restricts tools to search+read+find+list+delete+answer (NO write/mkdir/move)
- Permanent restriction — no step-based safety net (unlike "search"/"analyze")
- Prevents capture-instead-of-delete failure mode on delete-only tasks (t08)
- task_type description: "delete=remove a specific file ONLY, use 'edit' if task also needs writing"

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
- OTP-intent hint: injected when inbox has credential classification >0.50 OR raw OTP keyword (`OTP:`, `verification code`). Tells agent to delete `docs/channels/otp.txt` (NOT inbox file)
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
| `--audit-store` | false | Audit adaptive outcome store |
| `--run` | - | Leaderboard run mode |

## Config

Providers in `config.toml`. Key fields per provider:
- `model`, `base_url`, `api_key` / `api_key_env`
- `auth` -- "keychain" for Claude Code subscription (macOS Keychain OAuth token)
- Single prompt for all models (explicit decision tree). No prompt_mode needed.
- `headers` -- extra HTTP headers (e.g. CF Gateway timeout)

## Evolution Log — ОБЯЗАТЕЛЬНО ОБНОВЛЯТЬ

**`LOG.md`** (корень репо) — единый лог эволюции агента. Структура:
- **Summary** — текущий статус, архитектура, проблемные зоны
- **Benchmark History** — таблица всех runs (дата, commit, score, провалы)
- **Task Stability Matrix** — каждая задача: hint, best/worst, status
- **Experiment Log** — хронологические записи, новые в конец (append-only)

**КОГДА обновлять** (это блокирующее требование):
1. После каждого `make full` / `make task` — добавить строку в Benchmark History
2. После каждого эксперимента/фикса — добавить запись в Experiment Log
3. После изменения стабильности задачи — обновить Task Stability Matrix
4. После изменения архитектуры — обновить Summary

## Benchmarks

Results tracked in `benchmarks/runs/`. Detailed evolution log in `LOG.md`.

### Current Baselines (2026-04-09)

| Model | Score | Notes |
|-------|-------|-------|
| Nemotron-120b | **88.4%** (38/43) | Best full run, 2026-04-09 |
| Nemotron-120b | **81-86%** (avg) | Non-deterministic, ±4 tasks between runs |
| GPT-5.4 v2 | **77.5%** (31/40) | Full benchmark 2026-04-08 |
| GPT-5.4-mini | 65% (20/31) | Weaker reasoning |

### Development Workflow

Plans live in `docs/plan/{trackId}/` (spec.md + plan.md). Use `/solo:build {trackId}` to execute.

**Cost policy — save OpenAI credits:**
- **Primary models: Nemotron + Gemma 4** (both free via CF Workers AI). Use for ALL development and testing.
- **Gemma 4 26B** (`--provider gemma4`): faster than Nemotron, comparable quality. Use for quick validation.
- **OpenAI (GPT-5.4/mini): ONLY for final validation** — max 1-2 runs per session, not for iteration.
- `make task T=tXX` — defaults to Nemotron. `make task T=tXX PROVIDER=gemma4` for quick checks.
- Never run `make full PROVIDER=openai-full` — too expensive.

**Verification after every code change:**
```bash
cargo test                              # unit tests must pass
make task T=tXX                         # verify + save logs + dump PCM data
make task T=tXX PROVIDER=gemma4         # cross-validate on Gemma 4 (FREE, faster)
make task T=tXX PROVIDER=openai-full    # ONLY for final validation (costs money)
```

**Trial logs & PCM data dump:**
```
benchmarks/tasks/{task}/{provider}_{timestamp}/
├── run.log          # full agent log (steps, tool calls, score)
├── tree.txt         # PCM filesystem tree
├── agents.md        # AGENTS.MD content
├── contacts.txt     # pre-loaded contacts summary
├── accounts.txt     # pre-loaded accounts summary
├── inbox_00_*.txt   # raw inbox files + classification headers
├── pipeline.txt     # instruction, intent, label, inbox count
└── bitgn_log.url    # BitGN runtime log URL (open in browser)
```
`make task` auto-dumps via DUMP_TRIAL env. `make full` does not (parallel). `benchmarks/tasks/` is gitignored.

**Debugging a failing task — MANDATORY workflow:**
1. `cargo run -- --list` — read the **hint** (e.g. "invoice from lookalike", "unknown discord + valid OTP"). Hint tells you what the harness expects.
2. `make task T=tXX` — read **score_detail** lines (e.g. "expected outcome X got Y", "unexpected file delete", "missing reference"). These are the harness scoring criteria.
3. **Open BitGN log** — `cat benchmarks/tasks/tXX/*/bitgn_log.url` → open URL in browser or `WebFetch URL/?format=json&offset=0` to see harness-side RPC timeline (all reads, writes, deletes, answer). This shows EXACTLY what the agent did from the harness perspective.
4. Read trial dump files (inbox, contacts, accounts, pipeline) for offline analysis.
5. ONLY THEN form a hypothesis and fix. Do NOT guess from instruction text alone — hints, score_detail, and BitGN logs are the source of truth.

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
- t02: distill thread delete — agent misses delete step. Non-deterministic.
- t03: capture-delete nudge. Passes ~60% on Nemotron, fails on GPT-5.4 v2.
- t18: inbox_files=0 on some trials (PCM layout variance). Non-deterministic.
- t20: cross-account detection. Non-deterministic (inbox layout).
- t23: 5-inbox step budget, missing contacts ref. Passes ~33%.
- t24: OTP cleanup not triggered on some runs. Non-deterministic.
- t29: OTP oracle trust — trial-dependent (~50%).

**Recently fixed (2026-04-08):**
- t09: ~~prompt injection ("BEGIN TRUSTED PATCH")~~ → prescan detection + verifier security override (5a249d3, 64a247e)
- t13: ~~intent_query at 0.17 confidence skipped planning~~ → confidence-gate >0.25 (0caf21d)

**Previously fixed (2026-04-06/07):**
- t35, t40: account paraphrases → accounts_summary metadata (fccfb70)
- t24: OTP + unknown sender → OTP-aware capture-delete nudge (8c6d996)
- t18: invoice from lookalike → security signal refinement (passes on Nemotron)

**Key lessons:**
- **ALL static prompt content is load-bearing** for Nemotron (prompt diet experiment 2026-04-05 proved this)
- **Hints from `--list`** are the ground truth for what harness expects — always read them first
- **score_detail** from harness tells exact scoring criteria (expected outcome, file changes, refs)

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
