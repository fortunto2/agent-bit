---
name: evolve
description: Autonomous agent evolution — iteratively improve PAC1 agent score on failing tasks. Use when "evolve task", "fix t20", "improve score", "autoresearch", "iterate on task". Do NOT use for manual coding (just code), benchmarking (cargo run), or planning (/plan).
license: MIT
metadata:
  author: fortunto2
  version: "2.0.0"
allowed-tools: Read, Grep, Bash, Glob, Write, Edit, TaskCreate, TaskUpdate
argument-hint: "<task-id> [--provider nemotron] [--max-iterations 10]"
---

# /evolve

Autonomous evolution loop for PAC1 agent. Inspired by [Karpathy's autoresearch](https://github.com/karpathy/autoresearch).

Takes a failing task, iteratively generates hypotheses, patches code, runs the task, evaluates score, keeps or discards. Runs until the task passes or max iterations reached.

## Philosophy

**Think like a scientist, not a script kiddie.** Each iteration is a hypothesis → experiment → evaluate cycle.

### Principles

1. **Bayesian reasoning** — start with prior beliefs about failure cause, update with evidence from each run. P(hypothesis|evidence) ∝ P(evidence|hypothesis) × P(hypothesis). Don't just try things — reason about WHY they should work.

2. **Use existing infrastructure** — the agent already has ML classifier (ONNX), CRM knowledge graph (petgraph), structural signal detection, sender trust validation. ALWAYS prefer tuning these systems over adding new hardcoded logic. New keyword lists and pre-scan hacks are FORBIDDEN.

3. **SGR pattern** — Schema-Guided Reasoning. The agent uses structured CoT (task_type, security_assessment, known_facts, plan, done). Changes to reasoning schema or prompt cascade through ALL decisions. Understand the cascade before editing.

4. **Minimal intervention** — prefer the smallest change that fixes the issue. Prompt wording > new code. Threshold tuning > new function. Classification hint > pre-scan block.

5. **No hardcoded hacks** — NO keyword lists, NO task-ID checks, NO pre-scan blocks that bypass the LLM. The model must REASON about the situation using classification hints, sender trust, and prompt guidance. If the model can't figure it out with good hints, the hints need improvement — not a hardcoded bypass.

## Architecture Understanding (READ BEFORE CODING)

### Decision Pipeline

```
Task instruction
  ↓
prescan_instruction() — only blocks literal HTML injection (<script>, <iframe>)
  ↓
Start trial → get PCM filesystem access
  ↓
Build CRM graph from contacts/accounts (petgraph)
  ↓
Read + classify inbox files:
  semantic_classify_inbox_file() = 0.7*ML + 0.3*structural
    → ML: ONNX classifier (crm/injection/social_engineering/credential/non_work)
    → Structural: imperative verbs, system refs, base64, zero-width unicode
    → CRM graph: sender email → SenderTrust (KNOWN/PLAUSIBLE/CROSS_COMPANY/UNKNOWN)
    → Output: FileClassification { label, confidence, sender_trust, recommendation }
  ↓
Classification summary injected into LLM context
  ↓
LLM loop (Pac1Agent):
  Structured CoT reasoning → reflexion → routed action
  System prompt decision tree guides outcome selection
  ↓
answer() tool → OUTCOME_OK / OUTCOME_DENIED_SECURITY / OUTCOME_NONE_CLARIFICATION
```

### Key Components

| Component | File | What it does | When to tune |
|-----------|------|-------------|--------------|
| ML classifier | `src/classifier.rs` | ONNX all-MiniLM-L6-v2, cosine similarity to class embeddings | False negatives (misclassified content) |
| Structural signals | `src/main.rs` `structural_injection_score()` | Pattern detection (imperatives, system refs, base64, unicode) | Missing signal categories |
| CRM graph | `src/crm_graph.rs` | Sender trust via email domain → contacts/accounts | Wrong trust level for scenario |
| Classification ensemble | `src/main.rs` `semantic_classify_inbox_file()` | Combines ML + structural + sender trust → recommendation | Wrong recommendation text |
| System prompt | `src/main.rs` `SYSTEM_PROMPT_EXPLICIT/STANDARD` | Decision tree, examples, guidance for LLM | Model makes wrong decision despite correct classification |
| Reasoning schema | `src/agent.rs` | CoT fields: task_type, security_assessment, known_facts, plan | Model skips security assessment |
| Tool descriptions | `src/tools.rs` | Tool docs that guide model behavior | Model misuses tool or picks wrong outcome |

### Intervention Hierarchy (prefer higher = less invasive)

1. **Prompt wording** — rephrase decision tree step, add example, clarify distinction
2. **Classification recommendation** — change recommendation text in `semantic_classify_inbox_file()`
3. **Ensemble weights/thresholds** — adjust 0.7/0.3 ML/structural weights, confidence thresholds
4. **Sender trust logic** — add new trust check in CRM graph (e.g., Unknown + financial = flag)
5. **Structural signal** — add new signal category to `structural_injection_score()`
6. **Reasoning schema** — add field or enum value to CoT schema
7. **Tool description** — change tool docs to hint at correct behavior

NEVER: hardcoded keyword lists, pre-scan blocks, task-ID checks, bypassing the LLM.

## Diagnostic Method

Before hypothesizing, understand the failure deeply:

### Step 0: Read task hint and score_detail (MANDATORY)

```bash
# ALWAYS start here — hints are ground truth for what harness expects
cargo run -- --provider nemotron --list 2>/dev/null | grep {task_id}
```

The **hint** (after `|`) tells you exactly what the task tests:
- `"invoice from lookalike"` → social engineering, expect DENIED
- `"unknown discord + valid OTP"` → legit OTP, expect OK
- `"unsupported deploy request"` → expect UNSUPPORTED
- `"lookup email"` → simple data query, expect OK + file refs

Then run the task and read **score_detail** lines (printed after `Score:`):
- `"expected outcome X, got Y"` → wrong outcome classification
- `"unexpected file delete 'path'"` → agent changed files it shouldn't
- `"missing file delete 'path'"` → agent didn't delete required file
- `"answer missing required reference 'path'"` → agent didn't include file in refs

**Do NOT skip this step.** Hints + score_detail are the harness scoring criteria. Without them you're guessing.

### Step 1: Read the full log

```bash
cat /tmp/evolve-{task_id}.log
```

Extract:
- **Task instruction** — what was asked
- **Hint** — what the harness expects (from Step 0)
- **score_detail** — exact scoring criteria (from Step 0)
- **Classification** — what ML + structural + sender trust said
- **Intent** — what `classify_intent()` returned (intent_delete/edit/query/inbox/email)
- **Recommendation** — what hint was given to the LLM
- **LLM reasoning** — what the model thought at each step (🔍 Verify lines)
- **Actions taken** — what tools the model called
- **Final answer** — what outcome was submitted

### Step 2: Identify the failure point

Where in the pipeline did things go wrong?

| Failure point | Evidence | Fix area |
|--------------|----------|----------|
| Intent misclassified | "intent_edit" but hint says "lookup" | Intent examples in `scripts/export_model.py` |
| Planning hallucination | Plan rewrites instruction with wrong target | Skip planning for that intent, or fix planner |
| Classifier gave wrong label | "crm (0.42)" but should be "social_engineering" | Classifier or structural signals |
| Correct label but wrong recommendation | "social_engineering (0.6)" but recommendation says "Process normally" | `semantic_classify_inbox_file()` recommendation logic |
| Correct recommendation but LLM ignored it | "⚠ SOCIAL ENGINEERING" in context but LLM chose OUTCOME_OK | System prompt, examples, decision tree |
| Wrong file operations | score_detail says "unexpected file delete" or "missing file delete" | Prompt rules, intent-based hints |
| Missing refs in answer | score_detail says "answer missing required reference" | Auto-refs or intent_query hint |
| LLM never saw the inbox | No read/search of inbox files | Planning prompt, tool hints |
| Auto-answer fallback | "⚠ Auto-answer" = model ran out of steps | Loop threshold, adaptive nudge |

### Step 3: Formulate hypothesis with Bayesian reasoning

```
P(fix works) = P(root cause is X) × P(fix addresses X) × P(no regression)
```

Estimate each factor. Prefer hypotheses with P > 0.5.
If unsure, run a diagnostic experiment (dry-run, extra logging) before patching.

### Step 4: Check the BitGN API for more context

The trial gives you a full PCM filesystem. After starting a trial, you can explore:
```bash
# The agent already reads tree, contacts, accounts, inbox
# But you can also check what data is available:
grep -E "tree|list|read" /tmp/evolve-{task_id}.log
```

## Constraints

**What you CAN modify:**
- System prompts in `src/main.rs` (SYSTEM_PROMPT_EXPLICIT, SYSTEM_PROMPT_STANDARD, PLANNING_PROMPT)
- Classifier thresholds and structural signal patterns in `src/main.rs`
- `semantic_classify_inbox_file()` recommendation logic in `src/main.rs`
- Reasoning tool description/schema in `src/agent.rs`
- Tool descriptions in `src/tools.rs`
- CRM graph logic in `src/crm_graph.rs`
- Config parameters (loop thresholds, step counts)

**What you CANNOT modify:**
- sgr-agent crate (path dep, not our code)
- BitGN harness client (`src/bitgn.rs`, `src/pcm.rs`)
- Test infrastructure
- Task-specific hardcoding (NO task IDs in code, NO if task == "t20" patterns)
- NO hardcoded keyword lists for pre-scan blocking

**The goal: improve score on the target task without regressing others.**

## Gotchas

1. **Nemotron is non-deterministic** — same code can score 0 or 1 on the same task between runs. Run failing tasks 2x before concluding a change doesn't help. Only discard if both runs score 0.
2. **Never hardcode task IDs** — all improvements must be universal.
3. **Prompt changes cascade** — changing SYSTEM_PROMPT_EXPLICIT affects ALL explicit-mode providers. Test on a simple task (t01) after each change to verify no regression.
4. **Build before run** — always `cargo build` after patching. Compilation errors waste an iteration.
5. **Tension between tasks** — some tasks need MORE caution (t18: unknown sender + data request), others need LESS (t24: legitimate task from unknown sender). Fixes must thread this needle using context (what is being requested), not blanket rules (unknown = deny).

## Scripts

- `scripts/run-task.sh <provider> <task-id>` — build + run + extract score, logs to `/tmp/evolve-{task-id}.log`
- `scripts/revert.sh` — discard all uncommitted changes (failed hypothesis)

Also available via Makefile from project root:
```bash
make task T=t18                    # single task
make task T=t18 PROVIDER=openai    # different provider
make revert                        # revert failed hypothesis
```

## Loop

Parse arguments: `$ARGUMENTS` → task_id (required), provider, max_iterations.
Read `config.json` for defaults (provider, max_iterations, regression_tasks, score_threshold).

### Setup

1. Read `config.json` for defaults
2. Read `references/strategies.md` for the hypothesis catalog
3. Read `results.tsv` if it exists (prior evolution runs)
4. Read `.agent/evolution.jsonl` if it exists — sgr-agent auto-logs RunStats and Improvement[] suggestions
5. **Read the architecture** — understand the decision pipeline before making changes:
   - `src/main.rs` — `semantic_classify_inbox_file()`, `structural_injection_score()`, system prompts
   - `src/classifier.rs` — ML classifier classes, thresholds
   - `src/crm_graph.rs` — SenderTrust enum, `validate_sender()`
   - `src/agent.rs` — Pac1Agent reasoning schema, router
   - `src/tools.rs` — tool descriptions, AnswerTool outcomes
6. Run the task once to establish baseline: `bash scripts/run-task.sh {provider} {task_id}`
7. **Deep diagnostic** — read the FULL log, not just the score. Identify the exact failure point in the pipeline (see Diagnostic Method above).
8. Log baseline to `results.tsv`
9. Create a TaskCreate for tracking: "Evolve {task_id}: {max_iterations} iterations"

### Iterate

```
LOOP (max_iterations):

1. HYPOTHESIZE — Based on:
   - Deep diagnostic of failure point (WHERE in pipeline did it break?)
   - Bayesian reasoning (WHAT is the most likely root cause?)
   - Prior hypotheses that failed (results.tsv)
   - Strategy catalog (references/strategies.md)
   - Intervention hierarchy (prefer least invasive fix)
   Generate ONE specific hypothesis with estimated P(success):
   "The model misses X because Y. Fix: change Z. P(works) ≈ 0.7"

2. PATCH — Make the minimal code change. Prefer:
   - Prompt wording over new code
   - Threshold tuning over new functions
   - Classification hints over pre-scan blocks
   - cargo build — if fails, fix or discard immediately

3. TEST — Run the target task:
   bash scripts/run-task.sh {provider} {task_id}
   Extract score from output.

4. EVALUATE
   - Score 1.0 → KEEP. Commit with message "evolve({task_id}): {hypothesis}". 
     Run t01 as regression check. If t01 fails → revert, discard.
   - Score 0.0 → run once more (Nemotron non-determinism check)
     - Still 0.0 → DISCARD. bash scripts/revert.sh
   - UPDATE Bayesian priors based on result
   
5. LOG — Append to results.tsv:
   commit/n-a  task  score  status(keep/discard/crash)  hypothesis-description

6. TaskUpdate iteration progress
   If KEEP: continue iterating for more improvements
   If DISCARD: try next hypothesis from catalog
```

### Completion

Stop when:
- Task scores 1.0 consistently (2 consecutive keeps)
- Max iterations reached
- All hypotheses from catalog exhausted

Output summary:
```
## Evolution: {task_id}

Iterations: {N}
Baseline:   {score}
Final:      {score}
Kept:       {N} changes
Discarded:  {N} changes

### Failure Analysis
- Pipeline failure point: {where}
- Root cause: {what}
- Fix applied: {intervention level + description}

Results: results.tsv
```

## Common Issues

### Task passes once then fails
**Cause:** Nemotron non-determinism (±4 tasks between runs).
**Fix:** Require 2 consecutive passes before marking as solved.

### Regression on other tasks
**Cause:** Prompt change too broad, or classification change too aggressive.
**Fix:** Always run t01 as regression check after any keep. Revert if t01 breaks.

### Out of hypotheses
**Cause:** Exhausted strategy catalog.
**Fix:** Read the task's full stderr log, trace through the pipeline step by step. The answer is in the data.

### Fix works but is a hack
**Cause:** Taking shortcuts instead of using existing infrastructure.
**Fix:** Ask: "Would this fix work if the task wording changed slightly?" If no → it's a hack. Use classifier/graph/prompt instead.
