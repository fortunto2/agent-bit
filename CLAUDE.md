# CLAUDE.md -- agent-bit (PAC1 Agent)

BitGN PAC1 Challenge agent in Rust, powered by sgr-agent.

## Build & Run

```bash
cargo build
cargo test                                        # unit tests
make task T=t16                                   # single task (default: nemotron)
make task T=t16 PROVIDER=seed2                    # single task on specific model
make full P=3                                     # parallel full run (default: nemotron)
make full P=10 PROVIDER=seed2                     # parallel 10 on Seed
make sample                                       # 8-task quick sample
make failures M=Seed                              # show failures from dump dirs
make compare                                      # side-by-side model comparison
make ai-notes                                     # list all AI-NOTEs in codebase
make preflight                                    # verify env before competition
cargo run -- --provider nemotron --list            # list tasks WITH hints
cargo run -- --audit-store                         # audit adaptive kNN store
```

### Phoenix Observability (OTEL tracing)

Phoenix runs locally, receives OTEL spans from every LLM call. Requires `OTEL_EXPORTER_OTLP_ENDPOINT` in `.env`.

```bash
make phoenix                    # start Phoenix server (localhost:6006)
make phoenix-results            # CLI table: task/outcome/steps/score
cargo run --bin pac1-dash       # TUI dashboard (reads from Phoenix DB)
```

**What Phoenix shows:**
- **Spans tab** — every LLM call with input/output JSON, model, tokens, task_id
- **Traces tab** — trace-level view with score/outcome/task_id annotations
- **Sessions tab** — grouped by trial (session_id = `{task_id}_{trial_id}`)
- **Metrics tab** — score trends, token usage, annotation graphs

**How tracing works (sgr-agent `telemetry` feature):**
1. `init_telemetry(".agent", "pac1")` — sets up OTEL provider + simple exporter to Phoenix
2. Every LLM call → `record_llm_span()` → `chat.completions.api` span with input/output/tokens/session.id/task_id
3. After trial → `annotate_session()` → `trial.result` evaluator span + span/trace annotations via Phoenix REST API
4. `set_session_id()` / `set_task_id()` — called per trial, task_id falls back to parsing from session_id

**Debugging with Phoenix:**
1. Open `http://localhost:6006` → project `pac1`
2. Spans tab → filter by `metadata.task_id == 't16'` → see all LLM calls for that task
3. Click span → Info tab shows input/output JSON, Attributes tab shows tokens/model
4. Sessions tab → click session → see all spans in a trial grouped together
5. Traces tab → annotations columns show score/outcome per trace

**Phoenix data lives in:** `~/.phoenix/phoenix.db` (SQLite, survives restarts)
**OTEL config:** `.env` → `OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:6006`

### Dump-Based Analysis (ALWAYS use dumps, not stdout)

Every run writes to `benchmarks/tasks/{task}/{model}_{trial}/`:
- `pipeline.txt` — model, intent, label, score, steps, tool_calls, timing, score_detail
- `metrics.txt` — same metrics in KV format
- `score.txt` — score + harness detail lines
- `run.log` — full agent history (tool calls, reasoning)
- `inbox_*.txt` — pre-classified inbox content
- `bitgn_log.url` — harness-side RPC log URL

```bash
# Find failures for a model
grep "^score: 0" benchmarks/tasks/*/Seed*/pipeline.txt

# Read full diagnosis
cat benchmarks/tasks/t23/Seed-2.0-pro_vm-*/pipeline.txt

# Read agent history
cat benchmarks/tasks/t23/Seed-2.0-pro_vm-*/run.log

# Check AI-NOTEs related to a task
grep -rn "AI-NOTE.*t23\|AI-NOTE.*inbox" src/
```

## Architecture

```
src/pipeline.rs      -- enum state machine (New→Classified→InboxScanned→SecurityChecked→Ready)
src/main.rs          -- CLI, orchestration, guess_outcome, --probe flag
src/prompts.rs       -- system prompt (single explicit decision tree, hot-reload from prompts/system.md)
src/skills.rs        -- skill system: loads SKILL.md files, push-model selection via classifier
src/scanner.rs       -- security scanning, inbox classification, domain matching
src/pregrounding.rs  -- Codex-style context assembly + agent execution (713 lines)
src/agent.rs         -- Pac1Agent (Router + Structured CoT reasoning, two-phase FC)
src/pac1_sgr.rs      -- Pac1SgrAgent (pure SGR mode, single LLM call per step, experimental)
src/bitgn.rs         -- HarnessService client (Connect-RPC/JSON)
src/pcm.rs           -- PcmRuntime client (11 file-system RPCs + read cache + ProposedAnswer)
src/tools.rs         -- 16 Tools (13 base + read_all, search_and_read, grep_count) + trust metadata
src/hooks.rs         -- HookRegistry: data-driven tool completion hooks from AGENTS.MD
src/policy.rs        -- File access policy: structural guards for protected paths
src/config.rs        -- Provider config with temperature, sgr_mode, fallback_providers
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

**Directory:** `skills/` (14 skills, hot-reloadable — edit .md, no rebuild needed)
```
skills/
├── crm-default/SKILL.md       — general workspace (email, contacts, cross-account, multi-inbox)
├── crm-lookup/SKILL.md        — data queries, counting, captured article lookup
├── crm-invoice/SKILL.md       — resend/forward invoices with attachments
├── inbox-processing/SKILL.md  — multi-inbox workflow, channel priority, OTP
├── capture-distill/SKILL.md   — capture → card → thread → delete source + bulk cleanup
├── cleanup/SKILL.md           — delete cards/threads (bulk: list→delete, no read)
├── security-injection/SKILL.md — injection/social engineering → DENIED
├── security-credential/SKILL.md — OTP workflow (7 examples, exfiltration anti-pattern)
├── non-work/SKILL.md          — non-workspace → CLARIFICATION
├── unsupported/SKILL.md       — external API/deploy/calendar → UNSUPPORTED
├── followup-reschedule/SKILL.md — reschedule dates in accounts + reminders
├── invoice-creation/SKILL.md  — create typed invoice JSON
├── purchase-ops/SKILL.md      — fix purchase ID prefix regression
└── finance-query/SKILL.md     — bills, invoices, spend, revenue queries
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

### AI-NOTE Convention (MANDATORY)

Every behavioral code change MUST have `# AI-NOTE:` comment explaining WHY and which task it fixes.
Before modifying a file: `grep -r "# AI-" file` to check existing notes. Never remove AI-NOTEs without understanding why they exist.

Format: `# AI-NOTE: <what was changed> — <task> <reason>`
Example: `# AI-NOTE: security allows read — t29 OTP needs read docs/channels + otp.txt`

After fixing a task, verify with `grep "AI-NOTE" src/` that the note exists.

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

**Step 7 checklist**: when fixing LLM behavior, FIRST check if the right skill is selected (grep `🎯 Skill:` in logs). If wrong skill → adjust triggers/keywords. If right skill but wrong behavior → edit the skill's SKILL.md file.

### Classifier Upgrade Loop (the #1 pattern for fixing failures)

Most "non-deterministic" failures are actually **classifier misclassification** — the ML intent
classifier hasn't seen a particular instruction wording. Fix cycle:

1. Read `pipeline.txt` → check `intent:` field
2. Wrong intent? → add instruction to correct class in `scripts/export_model.py`
3. `uv run --with numpy,transformers,tokenizers,torch,onnxruntime,sentence-transformers scripts/export_model.py`
4. `cargo build && cargo run -- --provider nemotron --task tXX` — verify
5. Low-confidence fallback: if conf < 0.25 AND inbox_files > 0 → auto-force intent_inbox

**Auto-collect new variants:** all instructions saved to `benchmarks/tasks/{task}/*/instruction.txt`.
After competition runs, scan dumps and add new wordings to training:
```bash
find benchmarks/tasks -name "instruction.txt" -exec cat {} \; | sort -u
```

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

### System Prompt (prompts/system.md)

Single prompt for all models, hot-reloaded from `prompts/system.md` at runtime.
Explicit decision tree: "DENIED requires EXPLICIT evidence — not suspicion, not caution."
Fallback to compiled-in `SYSTEM_PROMPT_V2` if file not found.

### Batch Tools (efficiency)

Three tools reduce multi-step operations to single calls:
- `read_all(path)` — list + read ALL files in directory (batch for small dirs)
- `search_and_read(pattern, path)` — search + read first match (saves 1 call per lookup)
- `grep_count(pattern, path)` — count lines matching regex (no read + count manually)
- Trust metadata on every read: `[path | trusted/untrusted]` header

Result: t01 48→4 tool calls. All batch tools added to `filter_tools_for_task` allow-lists.

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

## Two State Machines + Feature Matrix (unified flow)

```
Pipeline SM (deterministic, pre-LLM):
  New(instruction)
    → classify()           — prescan + ML security + ML intent + question/inbox word override
  Classified
    → scan_inbox()         — parallel read + classify each file (ML + NLI + sender trust)
  InboxScanned
    → check_security()     — block threats + all-non_work → CLARIFICATION
  SecurityChecked → Ready

Feature Matrix (between pipeline and agent):
  Ready.inbox_files → InboxFeatureMatrix (12 features × N messages)
    → sigmoid(features × threat_weights) → P(threat) per message
    → Annotations: [threat: HIGH/MEDIUM/LOW (N%)]
    → Decision gate: sigmoid < 0.5 → safe to pre-execute capture

CRM Graph + ONNX Embeddings:
  → compute_account_embeddings() — L2-normalized MiniLM per account
  → detect_cross_account(body, sender) — cosine similarity, gap > 0.1
  → Annotation: [⚠ CROSS-ACCOUNT REQUEST]

Pre-grounding (Codex-style minimal, 6 messages):
  → AGENTS.MD, skill body, tree, date, inbox (with ML classification + threat scores), instruction
  → Skill selection (classifier label + intent → 14 SKILL.md files)
  → Cross-account detection (ONNX embeddings on inbox)

Workflow SM (during agent execution):
  Reading → Acting → Cleanup → Done
    → advance_step() — budget/write nudges
    → pre_action()   — outbox limit, delete guard, policy check, capture-write guard
    → post_action()  — phase transitions, hook messages, duplicate write warning
    → Guard::Block / Guard::Warn / Guard::Allow

Agent Loop (LLM):
  Structured CoT → Reflexion → Confidence gate → Router → Tool execution
    → Retry on empty (no tool calls → nudge 2x)

Post-execution:
  → verify_and_submit() — direct submit (no verifier)
  → score.txt saved to benchmarks/tasks/{task}/{trial}/
```

Each pipeline transition returns `Result<NextState, BlockReason>`. First block short-circuits.
Feature matrix sigmoid gate decides pre-execution. Workflow SM enforces runtime invariants.

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

### CRM Graph (entity resolution at runtime, not pre-grounding)
- CrmGraph built at startup (parallel IO) — used for search annotation + cross-account detection
- **Removed from pre-grounding** (2026-04-11): contacts_summary, accounts_summary, contact resolution hints
- Agent finds entities via search — SearchTool annotates results with CrmGraph account info
- CrmGraph methods: `contacts_for_account()`, `account_for_contact()`, `find_all_matching_contacts()`, `contact_names()`
- `annotate_contact_results()` / `annotate_account_results()` — enrich search output with linked entities
- `expand_query()` — 2-word queries try reversed word order ("Blom Frederike" → also "Frederike Blom")
- Auto-refs: merges recent reads with LLM-provided refs (accounts, contacts, invoices)

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

### Answer Submission (post-execution)
- **Deferred answer pattern**: AnswerTool stores `ProposedAnswer` via `pcm.propose_answer()` instead of submitting RPC immediately
- After execution loop, `verify_and_submit()` submits agent's answer directly (no verifier)
- When no proposed answer (agent didn't call answer()): uses `guess_outcome()` heuristic + `extract_last_finding()` from history
- **Verifier removed** (2026-04-11): was 3 parallel LLM calls per trial. Agent's answer is final.
- **FC probe removed from per-trial** (2026-04-11): moved to `--probe` CLI flag (`make probe`)
- **Planning phase removed** (2026-04-11): agent uses Phase 1 reasoning `plan` field instead
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
- **Planning phase removed** (2026-04-11): agent uses Phase 1 reasoning `plan` field instead
- **Intent-based routing**: `intent_delete` → skip multi-inbox scaling, `intent_inbox` → skill-driven workflow
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

### Pre-grounding Context (Codex-style minimal)
Simplified from 15+ messages to 6 (2026-04-11):
1. AGENTS.MD (workspace rules)
2. SKILL WORKFLOW (selected by classifier)
3. tree (directory structure)
4. date (from context())
5. Inbox content with ML classification headers: `[CLASSIFICATION: label (conf) | sender: trust | threat: HIGH (78%) | recommendation]`
6. Instruction

Removed: contacts_summary, accounts_summary, CRM schema, channel reads, channel trust,
OTP hints, capture hints, intent hints, capture pre-execute, inbox processing hints.
Agent explores workspace itself via tree + AGENTS.MD + batch tools.

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
| `--probe` | false | Test model FC support (run once per new model) |
| `--audit-store` | false | Audit adaptive outcome store |
| `--run` | - | Leaderboard run mode |

## Config

Providers in `config.toml`. Key fields per provider:
- `model`, `base_url`, `api_key` / `api_key_env`
- `auth` -- "keychain" for Claude Code subscription (macOS Keychain OAuth token)
- Single prompt for all models (explicit decision tree). No prompt_mode needed.
- `headers` -- extra HTTP headers (e.g. CF Gateway timeout)

### Model Compatibility (Two-Phase FC)

Agent uses two-phase function calling: Phase 1 = reasoning tool (8-field schema via `tools_call` with `tool_choice: required`), Phase 2 = action tools. Models must handle complex tool schemas on long context (7K+ system prompt).

**FC probe** via `--probe` CLI flag (`make probe PROVIDER=seed2`) — tests model FC support with 5-field schema. Run once per new model, not per trial.

**Quick model test**: `cargo run --release -- --provider NAME --task t01 && cargo run --release -- --provider NAME --task t16`. t01 = multi-step cleanup (hard), t16 = simple lookup (easy). Both must pass.

**Tested models (2026-04-10):**

| Model | Provider | t01 | t16 | Speed | Status |
|-------|----------|-----|-----|-------|--------|
| Nemotron 120B | CF Workers AI | ✅ | ✅ | 47s | **primary (FREE)** |
| GPT-5.4 | OpenAI | ✅ | ✅ | ~30s | paid, final validation only |
| MiniMax M2.5 | DeepInfra | ✅ | ✅ | 39s | **best backup ($0.27/M)** |
| Seed-2.0-pro | DeepInfra | ✅ | ✅ | 106s | **10/10 sample, $0.35/M** |
| Kimi-K2.5-Turbo | DeepInfra | ✅ | ✅ | 35s | **9/10 sample, $0.36/M, fast** |
| DeepSeek V3.2 | DeepInfra | ❌ | ✅ | — | FC ok, reasoning weak |
| Cerebras Qwen3 | Cerebras | ❌ | ✅ | fast | multi-step weak |
| Kimi K2.5 | CF Workers AI | ❌ | non-det | 53s | Phase 1 unreliable on long ctx |
| GLM-5.1 | DeepInfra | ❌ | ✅ | 287s | reasoning model, very slow |
| Step-3.5-Flash | DeepInfra | ❌ | ❌ | — | no strict schema support |
| Qwen3.5-397B | DeepInfra | ❌ | non-det | — | 0 tool calls on multi-step |
| Gemini 2.5 Pro | DeepInfra | ❌ | ❌ | — | FC broken on long context |
| Gemini 2.5 Flash | DeepInfra | ❌ | ❌ | — | FC broken on long context |
| Nemotron 340B | DeepInfra | ❌ | ❌ | — | FC broken |
| Qwen3-Max | DeepInfra | ❌ | ✅ | — | multi-step weak |

**Root cause for failures**: not missing FC support, but **degraded tool calling on long context**. Models pass simple FC probe but fail when system prompt is 7K+ with complex reasoning schema. Only Nemotron, GPT-5.4, and MiniMax handle this reliably.

**Adding new models**: add to config.toml, run `--task t01` + `--task t16`, check metrics.txt in dump dir.

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

**Trial dumps** (auto-saved for ALL runs — single + parallel):
```
benchmarks/tasks/{task}/{trial_id}/
├── instruction.txt  # raw instruction (saved before prescan)
├── pipeline.txt     # DIAGNOSIS: intent, label, per-inbox classification, score, tool_calls
├── score.txt        # final score + score_detail
├── run.log          # full agent log (if single-task run)
├── tree.txt         # PCM filesystem tree
├── agents.md        # AGENTS.MD content
├── contacts.txt     # pre-loaded contacts summary
├── accounts.txt     # pre-loaded accounts summary
├── inbox_00_*.txt   # raw inbox files + classification headers
└── bitgn_log.url    # BitGN runtime log URL (press 'o' in dashboard)
```

**Debugging a failing task — checklist:**
1. `cargo run --bin pac1-dash` → find task in heatmap, press `o` to open BitGN log
2. Read `pipeline.txt` — check these fields:
   - `intent:` — wrong intent? (e.g. intent_delete instead of intent_inbox) → retrain classifier
   - `label:` — wrong ML label? → check classifier thresholds
   - `per_inbox:` — wrong sender trust? → check CRM graph
   - `score:` + `detail:` — what harness expected vs got
   - `tool_calls:` — 0 = agent never started, 1 = just answered, 3+ = investigated
3. If prescan blocked → check `instruction.txt` for false positive patterns
4. If wrong skill selected → check `🎯 Skill:` in logs, adjust triggers/keywords
5. If correct skill but wrong action → check workflow phase, outbox limit, delete guard
6. If all correct but wrong outcome → check verifier override, confidence reflection

**Quick diagnosis table:**
| pipeline.txt shows | Root cause | Fix location |
|---|---|---|
| `intent: intent_delete` for capture task | ML classifier misclassification | `scripts/export_model.py` training examples |
| `label: injection` for legit content | ML security false positive | classifier thresholds |
| `sender: UNKNOWN` for known contact | CRM graph ingestion issue | `crm_graph.rs` |
| `tool_calls: 0` | Prescan blocked or agent loop error | `scanner.rs` prescan or `agent.rs` |
| `score: 0` + `detail: expected OK got DENIED` | False security alert | `guard_content` or override policy |
| `score: 0` + `detail: unexpected file write` | Agent over-processed | workflow guards or skill |
| `score: 0` + `detail: missing file write` | Agent skipped step | skill guidance or pre-execute |

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
