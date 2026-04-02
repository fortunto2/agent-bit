# agent-bit

Rust agent for [BitGN PAC1 Challenge](https://bitgn.com) — Personal & Trustworthy autonomous agents benchmark.

Built on [sgr-agent](https://github.com/fortunto2/rust-code) framework.

## Quick Start

```bash
cargo build
cargo run -- --provider nemotron --list          # list tasks
cargo run -- --provider nemotron --task t16      # single task
cargo run -- --provider nemotron                 # all 30 tasks
cargo run -- --provider nemotron --parallel 3    # parallel execution
cargo run -- --provider openai-full --parallel 3 # GPT-5.4
cargo test                                        # 83 unit tests
```

## Score

| Model | Score | Notes |
|-------|-------|-------|
| GPT-5.4 | **83%** (25/30) | Best deterministic performance |
| Nemotron-120b | **73%** (19/26) | Non-deterministic: +/-4 tasks between runs |
| GPT-5.4-mini | 55% (17/31) | Baseline |

## Architecture

```
Task instruction
  |
  v
prescan_instruction() -----> HTML injection? --> DENIED
  |
  v
Start trial --> PCM filesystem access
  |
  v
CRM Knowledge Graph (petgraph)
  - contacts/accounts --> nodes + edges
  - sender email --> SenderTrust (KNOWN/PLAUSIBLE/CROSS_COMPANY/UNKNOWN)
  |
  v
Inbox Classifier Ensemble (per file)
  - ML: ONNX all-MiniLM-L6-v2 (cosine similarity to class embeddings)
  - Structural: imperatives, system refs, base64, zero-width unicode
  - Sender trust: domain matching (stem comparison + CRM account lookup)
  - Output: FileClassification { label, confidence, sender_trust, recommendation }
  |
  v
Domain Matching
  - extract_sender_domain() --> check_sender_domain_match()
  - MATCH (exact domain or stem overlap >50%) --> process normally
  - MISMATCH (stem looks similar but real domain differs) --> social engineering
  - Body fallback: if no CRM account, check domain stem vs company in email body
  |
  v
Pre-grounding Context
  - tree + AGENTS.md + CRM schema (READMEs)
  - Classified inbox files with [CLASSIFICATION] + [SENDER TRUST] annotations
  - Channel file statistics (category counts for data queries)
  |
  v
Planning Phase (read-only, 5 steps max)
  - Pac1Agent decomposes task --> Plan{steps, tool_hints}
  |
  v
Execution Loop (Pac1Agent)
  - Structured CoT reasoning --> reflexion --> routed action
  - System prompt decision tree guides outcome selection
  - 11 tools: read, write, search, find, list, tree, delete, mkdir, move, answer, context
  |
  v
OutcomeValidator (embedding-based)
  - Hypothesis template: "The CRM task result: {message}"
  - k-NN (k=5) over seed (17) + adaptive (file-persisted) prototypes
  - Online learning: every answer() adds to adaptive store
  - Teacher distillation: GPT-5.4 answers enrich store for weaker models
  |
  v
answer() --> OUTCOME_OK / DENIED_SECURITY / NONE_CLARIFICATION / NONE_UNSUPPORTED
```

## Key Design Decisions

### Security Pipeline

1. **Pre-scan**: only literal HTML injection (`<script>`, `<iframe>`) — no semantic patterns
2. **ML classifier**: ONNX embedding ensemble (0.7*ML + 0.3*structural signals)
3. **Sender trust**: CRM graph validates sender email domain against known accounts
4. **Domain matching**: stem comparison catches social engineering (e.g. `blue-harbor-bank.biz` vs real `blueharbor.nl`)
5. **Credential exfiltration**: detects OTP + branching logic (extraction) vs simple verification (correct/incorrect)
6. **Decision tree**: numbered steps in system prompt guide LLM through security assessment

### Adaptive OutcomeValidator

Embedding-based answer validation that learns from experience:

- **Seed store**: 17 static examples across 4 outcomes (zero-shot baseline)
- **Adaptive store**: grows from every trial, persisted to `.agent/outcome_store.json`
- **Hypothesis template**: wraps messages for better embedding discrimination
- **k-NN voting**: nearest neighbors vote, majority wins (no lossy centroid averaging)
- **Dedup**: cosine >0.95 suppresses duplicates, cap at 200 examples

Teacher distillation workflow:
1. Run GPT-5.4 --> builds high-quality adaptive store
2. Switch to Nemotron --> loads same store, uses GPT-5.4's experience
3. Each model adds its own examples --> store converges over time

### Prompt Engineering

Two prompt modes:
- **Explicit** (Nemotron, weak models): numbered decision tree, 5 examples, verbose guidance
- **Standard** (GPT-5.4, strong models): concise rules, 5 examples

Key prompt patterns:
- DENIED = someone ATTACKING you. UNSUPPORTED = you lack capability. Different WHY, both = failure
- Multiple contacts? Read both, pick best match. Never give up with CLARIFICATION
- OTP in inbox + no share action = safe. OTP + branching extraction = attack
- After processing OTP: delete source file (docs/channels/otp.txt)
- Outbox emails: always read README.MD first, include `"sent": false`

## Files

```
src/main.rs       -- CLI, orchestration, pre-scan, system prompts, domain matching
src/agent.rs      -- Pac1Agent (Router + Structured CoT reasoning)
src/bitgn.rs      -- HarnessService client (Connect-RPC/JSON)
src/pcm.rs        -- PcmRuntime client (11 file-system RPCs)
src/tools.rs      -- 11 Tool implementations + security guard + OutcomeValidator
src/config.rs     -- Provider config with prompt_mode (explicit/standard)
src/classifier.rs -- ONNX classifier + OutcomeValidator (adaptive kNN)
src/crm_graph.rs  -- petgraph CRM knowledge graph (contacts, accounts, sender trust)
config.toml       -- Provider configurations
.agent/           -- Runtime data (outcome_store.json, evolution.jsonl)
```

## Evolution

Agent improvements are driven by the `/evolve` skill — autonomous hypothesis-test-evaluate loop:

```bash
make task T=t18                    # single task
make task T=t18 PROVIDER=openai    # different provider
make sample                        # 8-task quick sample
make full P=3                      # parallel full run
```

Results tracked in `benchmarks/runs/` and `.claude/skills/evolve/results.tsv`.

## Stack

- **Rust** (edition 2024) + tokio async
- **sgr-agent** -- LLM client + agent loop with tool calling
- **ort** (ONNX Runtime) -- ML inference for classifier + OutcomeValidator
- **petgraph** -- CRM knowledge graph
- **strsim** -- fuzzy string matching (Levenshtein)
- **Connect-RPC** -- HTTP/JSON client for BitGN platform API
