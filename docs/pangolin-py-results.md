# Pangolin-py experiment — results

Branch: `experiment/pangolin-py`. One-tool `execute_code` architecture,
Python subprocess runtime, scratchpad persistence, workspace RPC to
shared PcmClient. Ported from Operation Pangolin (BitGN PAC1 winner).

## Leaderboard results (104-task prod)

| Run | Provider | Score | Notes |
|-----|----------|-------|-------|
| v1  | Gemma4   | **58%** (60/104) | baseline, simple prompt |
| v2  | Gemma4   | 47% (killed early) | + `verify()`, gate-check, hard-block |
| v3  | Gemma4   | 39% (61/104 partial) | + self-validation prompt (too verbose) |
| v4  | Gemma4   | 42% | concise schema rule, kept validators |
| v5  | Gemma4   | 50% (51/102) | cleaner workspace, non-det drift |
| v6  | Gemma4   | 53% (55/104) | exact v1 rollback + `ws.prepend` |
| **v7**  | **Haiku**    | **54% (56/104)** | same prompt, stronger model |

Comparison to our **main** architecture (ML classifier + skills + feature matrix):

| Provider | Main | Pangolin-py | Delta |
|----------|------|-------------|-------|
| Gemma4   | 64%  | 58%         | -6pp  |
| Haiku    | 78%  | 54%         | **-24pp** |
| Opus*    | —    | 88%         | (hypothetical) |

*Opus in Pangolin original authors' blind run.

## What went wrong on Gemma4/Haiku

Same failure classes as reported by @i_november on Pangolin original:

- **Hidden injection** (t011-style) — 4× DENIED→OK misses in v7
- **Bulk delete** (t006-style) — 5× missing file write
- **Entity finding requiring LLM judgment** (t025, t051-style) — 4× "answer is incorrect, expected 0"
- **Date arithmetic** (t012, t037-style) — overlaps with wrong-value failures

These are **model-judgment weaknesses**, not architecture bugs.
Opus has them too but fewer; Haiku/Gemma4 fall into them more often.

## What we learned (v1 → v7)

**Anti-patterns — don't do these on mid-tier models:**

1. **Auto-validators in `ws.answer`**. Our gate-consistency check raised
   `ValueError` whenever `outcome=OK` coincided with any scratchpad key
   equal to `"NO"/"BLOCKED"`. Gemma4 interpreted the block as "can't
   submit OK" and retried with `CLARIFICATION` — pushed 5 legit-OK trials
   into wrong outcome.

2. **Hard-block on `ws.write(0,0)`**. Intended to force `ws.prepend`;
   actually broke legitimate rewrite flows.

3. **Self-validation prompt** (readback + yaml parse + diff keys). Mid-tier
   models burn extra iterations and hit max_iter; pass rate dropped to 39%.

**What helps (thin layer only):**

- `ws.prepend(path, header)` as sugar for `ws.write(path, header, 1, 1)`.
  Pure syntactic convenience, no new behavior.
- Auto-submit `OUTCOME_NONE_CLARIFICATION` when loop exits without
  `ws.answer`. Catches ~5-10 no-answer failures per run.
- Pre-parse `pcm.context()` as `{time, unixTime}` into `scratchpad.context`
  (same as Pangolin TS wrapper).

## Take-away

Pangolin-architecture delivers 88% **on Opus-tier models**. On Gemma4 and
Haiku it sits 6-24pp **below** our full-stack main agent, because the
single-tool-single-prompt design concentrates all the reasoning pressure
on the LLM — no ML pre-classifier, no skill routing, no feature-matrix
scoring to lean on. Weak judgment → weak trial.

**Recommended next direction**: add `execute_code` as a 17th tool to the
main agent (read-only flavor: `ws_read/list/search/find/tree/context`,
no write). Gives the main LLM a `compute` step for aggregation /
multi-file filter / counts — where main currently fails with
"wrong_answer_value" (12× on Gemma4 v1). Keeps our ML scaffolding intact.

Branch `experiment/pangolin-py` stays as reference implementation; don't
merge.
