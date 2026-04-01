---
name: evolve
description: Autonomous agent evolution — iteratively improve PAC1 agent score on failing tasks. Use when "evolve task", "fix t20", "improve score", "autoresearch", "iterate on task". Do NOT use for manual coding (just code), benchmarking (cargo run), or planning (/plan).
license: MIT
metadata:
  author: fortunto2
  version: "1.0.0"
allowed-tools: Read, Grep, Bash, Glob, Write, Edit, Agent
argument-hint: "<task-id> [--provider nemotron] [--max-iterations 10]"
---

# /evolve

Autonomous evolution loop for PAC1 agent. Inspired by [Karpathy's autoresearch](https://github.com/karpathy/autoresearch).

Takes a failing task, iteratively generates hypotheses, patches code, runs the task, evaluates score, keeps or discards. Runs until the task passes or max iterations reached.

## Constraints

**What you CAN modify:**
- System prompts in `src/main.rs` (SYSTEM_PROMPT_EXPLICIT, SYSTEM_PROMPT_STANDARD, PLANNING_PROMPT)
- Classifier thresholds and structural signal patterns in `src/main.rs`
- Reasoning tool description/schema in `src/agent.rs`
- Tool descriptions in `src/tools.rs`
- CRM graph logic in `src/crm_graph.rs`
- Config parameters (loop thresholds, step counts)

**What you CANNOT modify:**
- sgr-agent crate (path dep, not our code)
- BitGN harness client (`src/bitgn.rs`, `src/pcm.rs`)
- Test infrastructure
- Task-specific hardcoding (NO task IDs in code, NO if task == "t20" patterns)

**The goal: improve score on the target task without regressing others.**

## Gotchas

1. **Nemotron is non-deterministic** — same code can score 0 or 1 on the same task between runs. Run failing tasks 2x before concluding a change doesn't help. Only discard if both runs score 0.
2. **Never hardcode task IDs** — all improvements must be universal. "If inbox mentions company X" is fine. "If task is t20" is forbidden.
3. **Prompt changes cascade** — changing SYSTEM_PROMPT_EXPLICIT affects ALL explicit-mode providers. Test on a simple task (t01) after each change to verify no regression.
4. **Build before run** — always `cargo build` after patching. Compilation errors waste an iteration.
5. **Planning phase doubles latency** — if the task is simple, the planning phase adds overhead. Consider whether the task actually needs planning or would benefit from classifier/prompt changes instead.

## Loop

Parse arguments: `$ARGUMENTS` → task_id (required), provider (default: nemotron), max_iterations (default: 10).

### Setup

1. Read `references/strategies.md` for the hypothesis catalog
2. Read `results.tsv` if it exists (prior evolution runs)
3. Run the task once to establish baseline score
4. Log baseline to `results.tsv`

### Iterate

```
LOOP (max_iterations):

1. HYPOTHESIZE — Based on:
   - Task failure output (grep for "expected outcome", "missing file", error messages)
   - Prior hypotheses that failed (results.tsv — what was already tried)
   - Strategy catalog (references/strategies.md)
   Generate ONE specific hypothesis: "The model misses X because Y. Fix: change Z."

2. PATCH — Make the minimal code change. One file, few lines.
   - cargo build — if fails, fix or discard immediately

3. TEST — Run the target task:
   cargo run -- --provider {provider} --task {task_id} 2>&1 | tee /tmp/evolve-{task_id}.log
   Extract score from output.

4. EVALUATE
   - Score 1.0 → KEEP. Commit with message "evolve({task_id}): {hypothesis}". 
     Run t01 as regression check. If t01 fails → revert, discard.
   - Score 0.0 → run once more (Nemotron non-determinism check)
     - Still 0.0 → DISCARD. git checkout -- . Revert all changes.
   
5. LOG — Append to results.tsv:
   commit/n-a  score  status(keep/discard/crash)  hypothesis-description

6. If KEEP: celebrate, continue iterating for more improvements
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

Results: results.tsv
```

## Common Issues

### Task passes once then fails
**Cause:** Nemotron non-determinism (±4 tasks between runs).
**Fix:** Require 2 consecutive passes before marking as solved.

### Regression on other tasks
**Cause:** Prompt change too broad.
**Fix:** Always run t01 as regression check after any keep. Revert if t01 breaks.

### Out of hypotheses
**Cause:** Exhausted strategy catalog.
**Fix:** Read the task's full stderr log (`/tmp/evolve-{task_id}.log`), identify the exact failure point, generate a new hypothesis from the specific error.
