# agent-bit

Rust agent for [BitGN PAC1 Challenge](https://bitgn.com) — Personal & Trustworthy autonomous agents benchmark.

Built on [sgr-agent](https://github.com/fortunto2/rust-code) framework.

## Score

### Prod benchmark (pac1-prod, 104 tasks) — current v4 codebase

| Model | Score | Cost | Run |
|-------|-------|------|-----|
| **Haiku 4.5** (OpenRouter) | **81/104 (78%)** | ~$5 | [run-22JoCuyLMFd2z735su2bWbF4V](https://eu.bitgn.com/runs/run-22JoCuyLMFd2z735su2bWbF4V) |
| Nemotron 3 Super 120b (OR) | 67/104 (65%) | FREE | 2026-04-17 three-way |
| Gemma 4 26b (CF Workers) | 67/104 (64%) | FREE | [run-22Jxp5aALc71xiAaCgRxpDoBc](https://eu.bitgn.com/runs/run-22Jxp5aALc71xiAaCgRxpDoBc) |

**Current best: Haiku 4.5 at 78%.** Free-tier ceiling ≈ 64-65% (Nemotron/Gemma). Haiku adds +13-15pp for $5/run. GPT-5.4 on the v4 codebase is projected ~93% after t09+t18+t20+t23+t29 fixes but has not been re-run at that spend (last full run 04-13 predates ~20 universal fixes and is excluded as not comparable).

### Dev benchmark (pac1-dev, 43 tasks)

| Model | Score | Notes |
|-------|-------|-------|
| Nemotron 120B (CF) | **95.3%** (41/43) | FREE, primary for development |
| Seed-2.0-pro | 90.7% (39/43) | Best paid alternative |
| GPT-5.4 | 95.3% (41/43) | Expensive, final validation only |

30+ models tested across 6 providers (DeepInfra, CF Workers AI, Cerebras, OpenRouter, Modal, OpenAI). See `LOG.md` Benchmark History for full chronology.

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

Four layers, each owning one slice of responsibility:

```
┌──────────────────────────────────────────────────────────────────┐
│  L1: Pre-LLM Pipeline (deterministic, no model calls)            │
│  prescan → classify (security+intent) → scan inbox → check → ready
│  Files: pipeline.rs, scanner.rs, classifier.rs, intent.rs,       │
│         crm_graph.rs, feature_matrix.rs, policy.rs               │
├──────────────────────────────────────────────────────────────────┤
│  L2: Context Assembly (builds 6-message pre-grounding)           │
│  tree + AGENTS.MD + skill body + date + inbox-with-annotations   │
│  + instruction. Files: pregrounding.rs, skills.rs, hooks.rs      │
├──────────────────────────────────────────────────────────────────┤
│  L3: Agent Loop (LLM-driven, tool-calling)                       │
│  Pac1Agent: Structured CoT → Reflexion → Router → Tools          │
│  Enforced by Workflow SM guards at each tool call                │
│  Files: agent.rs, pac1_sgr.rs, tools.rs, workflow.rs             │
├──────────────────────────────────────────────────────────────────┤
│  L4: Post-execution (validation, learning, dumps)                │
│  OutcomeValidator kNN → adaptive store → trial dumps             │
│  Files: classifier.rs (OutcomeValidator), trial_dump.rs          │
└──────────────────────────────────────────────────────────────────┘
```

Dependencies flow one way: **L1 → L2 → L3 → L4**. Nothing in L1 imports L3.

### Key Components

| Component | File | Purpose |
|-----------|------|---------|
| **L1 — Pre-LLM** | | |
| Pipeline SM | [`src/pipeline.rs`](src/pipeline.rs) | Typed state machine `New→Classified→InboxScanned→SecurityChecked→Ready`. Pre-LLM classification + security signals. |
| Intent | [`src/intent.rs`](src/intent.rs) | Typed enum (`Inbox/Delete/Query/Edit/Email/Unclear`) with behavioral methods. Single source of truth replacing scattered string compares. |
| Classifier | [`src/classifier.rs`](src/classifier.rs) | ONNX MiniLM-L6 (security + intent) + NLI DeBERTa (zero-shot) + OutcomeValidator (adaptive kNN, 5-nearest). Returns typed `Intent`, WARNs on drift. |
| Scanner | [`src/scanner.rs`](src/scanner.rs) | Prescan (HTML injection), sender assessment, domain matching, exfiltration detection. |
| CRM Graph | [`src/crm_graph.rs`](src/crm_graph.rs) | petgraph entity graph + ONNX account embeddings + cross-account detection (cosine sim, gap > 0.1). |
| Feature Matrix | [`src/feature_matrix.rs`](src/feature_matrix.rs) | 12-feature × sigmoid → threat probability per inbox message. Ridge-regression calibration. |
| Policy | [`src/policy.rs`](src/policy.rs) | File protection (`PROTECTED_BASENAMES`, `POLICY_DIRS`), channel trust registry, exfiltration scan. |
| **L2 — Context** | | |
| Pregrounding | [`src/pregrounding.rs`](src/pregrounding.rs) | Codex-style 6-message assembly. Tree + AGENTS.MD + skill + inbox with classification headers. |
| Skills | [`skills/`](skills/) + [`src/skills.rs`](src/skills.rs) | 15 hot-reload `.md` skills, push-model selection via classifier label + intent. Trigger validation at load. |
| Hooks | [`src/hooks.rs`](src/hooks.rs) | Data-driven tool completion hooks parsed from AGENTS.MD (`path_contains` patterns). |
| System Prompt | [`prompts/system.md`](prompts/system.md) | Hot-reload, evolved via ShinkaEvolve. |
| **L3 — Agent** | | |
| Pac1Agent | [`src/agent.rs`](src/agent.rs) | Single- or two-phase LLM loop, Structured CoT, parallel think+action, router, reflexion. |
| Pac1SgrAgent | [`src/pac1_sgr.rs`](src/pac1_sgr.rs) | SGR-mode (1 LLM call/step) alternative to Pac1Agent. |
| Tools | [`src/tools.rs`](src/tools.rs) | 16 tools: read/write/delete/search/list/tree/grep_count/read_all/search_and_read + trust metadata. |
| Workflow SM | [`src/workflow.rs`](src/workflow.rs) | Runtime guards (`Reading→Acting→Cleanup→Done`): budget nudges, write limits, delete control, capture-write ordering. |
| PcmClient | [`src/pcm.rs`](src/pcm.rs) | Harness FS RPCs + read cache + `ProposedAnswer`. |
| **L4 — Post** | | |
| Outcome kNN | [`src/classifier.rs`](src/classifier.rs) (`OutcomeValidator`) | Score-gated adaptive store (`.agent/outcome_store.json`), 5-NN voting, confidence-gated block/warn. |
| Trial Dumps | [`src/trial_dump.rs`](src/trial_dump.rs) | `pipeline.txt`, `inbox_*`, `tree.txt`, `contacts.txt` for offline debug. |
| Dashboard | [`src/dashboard.rs`](src/dashboard.rs) | TUI: model columns, heatmap, log viewer (`cargo run --bin pac1-dash`). |

### Hot-Reload (zero rebuild)

- `prompts/system.md` — system prompt
- `skills/*.md` — 15 domain-specific skills
- `config.toml` — temperatures, providers, parallelism

## Reusable Patterns for Other Projects

Every agent-bit layer is a transplantable pattern. Most code is project-specific (domain labels, CRM schema, PAC1 outcomes) but the *shape* generalizes.

### Pattern 1 — Typed classifier labels with drift detection

**File:** [`src/intent.rs`](src/intent.rs) (~250 LOC). Pure enum with `parse` / `as_str` / `Display` / `Serialize`, plus behavioral methods (`forces_task_type`, `outbox_limit(is_capture)`, `allows_multi_write`).

**How to reuse:** copy `intent.rs`, rename variants to your domain, list wire-format strings in `wire_values()`. Wire the enum into:
- Classifier return type: `Vec<(YourLabel, f32)>` — emits WARN on unknown labels (catches ONNX/embedding drift)
- LLM JSON-schema: `"enum": YourLabel::wire_values()` — no manual sync on enum changes
- Skill YAML triggers: `validate_triggers()` warns on typos at load

Eliminates ~50 string compares per enum + ~3 latent bug classes. See PR [#1](https://github.com/fortunto2/agent-bit/pull/1).

### Pattern 2 — Pre-LLM pipeline state machine

**File:** [`src/pipeline.rs`](src/pipeline.rs) (typed `New→Classified→InboxScanned→SecurityChecked→Ready`).

**How to reuse:** each transition returns `Result<NextState, BlockReason>`. First block short-circuits → deterministic outcome without wasting LLM tokens. Compiler enforces ordering (can't skip security check). Use for any agent with pre-validation (auth, content moderation, classification gates).

### Pattern 3 — Runtime workflow state machine

**File:** [`src/workflow.rs`](src/workflow.rs) (`Reading→Acting→Cleanup→Done` + per-tool guards).

**How to reuse:** replaces scattered `if`-guards across 5+ files with one SM. `pre_action(tool, path) → Guard::{Block, Warn, Allow}` runs before each tool executes. `post_action` advances phase + fires hooks. Agent can't misbehave — guards return string responses the LLM sees instead of executing.

Key rule: **Block > Warn**. Weak models ignore warnings, obey blocks (messages injected as tool output).

### Pattern 4 — Skill-based prompt injection (hot-reload)

**Files:** [`skills/`](skills/) + [`src/skills.rs`](src/skills.rs) + `sgr-agent::skills`.

**How to reuse:** YAML-frontmatter Markdown files with `triggers` + `keywords` + `priority`. Push-model: classifier label + intent → skill body injected into `{examples}` prompt placeholder. Hybrid fallback: agent can `list_skills()` / `get_skill()` mid-task. Validation warns on typos. Works for any domain where you want "domain-specific examples without rebuild".

### Pattern 5 — Feature matrix for classification

**File:** [`src/feature_matrix.rs`](src/feature_matrix.rs) (12 features × sigmoid → probability).

**How to reuse:** batch scoring over N items with hand-tuned weights (`threat_weights()`) that ridge regression can calibrate from labels (`calibrate_ridge()` — Gauss-Seidel solver, R²=0.999). Correlation matrix exposes feature importance. Decision gate: `sigmoid < 0.5 → safe`. Works for any ensemble decision (spam, fraud, content quality).

### Pattern 6 — Data-driven hooks from AGENTS.MD

**File:** [`src/hooks.rs`](src/hooks.rs) (`HookRegistry::from_agents_md`).

**How to reuse:** parses natural-language rules (`"When adding to {path}, also {action}"`) into typed `ToolHook{tool, path_contains, message}`. Triggered by tool name + path match. Lets domain docs drive agent behavior without code changes.

### Pattern 7 — Adaptive outcome validation (kNN on past trials)

**File:** [`src/classifier.rs`](src/classifier.rs) (`OutcomeValidator`).

**How to reuse:** stores past outcomes by embedding in `.agent/outcome_store.json`. On new trial, k-NN vote (k=5) against store. Score-gated learning (only confirmed correct trials enter store). Confidence-gated block/warn. Self-improves without retraining a model.

### Pattern 8 — Tool trust metadata

**File:** [`src/tools.rs`](src/tools.rs) (read annotations like `[path | trusted/untrusted]`).

**How to reuse:** every read adds a header indicating whether the source is trusted (system file) or untrusted (inbox message). Prompt instructs model to treat differently. Prevents injection by making provenance visible in context.

### What you CAN'T transplant directly

- [`src/prompts.rs`](src/prompts.rs) / [`prompts/system.md`](prompts/system.md) — PAC1 decision tree, rewrite for your domain
- [`skills/`](skills/) — CRM-specific examples
- [`src/bitgn.rs`](src/bitgn.rs) / [`src/pcm.rs`](src/pcm.rs) — harness-specific RPC clients
- `models/*.onnx` + `models/class_embeddings.json` — trained on PAC1 task wordings

### Minimal transplant recipe

For a new agent using this pattern:

1. **L1 skeleton**: `pipeline.rs` + `intent.rs` + thin `classifier.rs` stub
2. **L2 skeleton**: `pregrounding.rs` assembling your domain context + `skills.rs` pointing at your `skills/` dir
3. **L3 skeleton**: pick `Pac1Agent` (two-phase, stronger) or `Pac1SgrAgent` (single-phase, 3× faster) as starting point
4. **L4**: trial dumps are literally 30 LOC, copy as-is
5. **Domain**: write `skills/*/SKILL.md`, adjust `Intent` variants, train your ONNX classifier via `scripts/export_model.py`

Expect 80% of `agent-bit` code to transplant; 20% is domain-specific.

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
