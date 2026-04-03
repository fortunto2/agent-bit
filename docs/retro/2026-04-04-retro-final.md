# Pipeline Retro: agent-bit (2026-04-04) — Final

## Overall Score: 5.7/10

Full-day pipeline: 7 tracks, 43 iterations, 48.8% waste across ~7h23m. Strong technical output (1 refactor + 6 bug fixes, 123 tests green, Nemotron 24→30 potential) undermined by the OAuth deploy spin-loop (14/21 wasted iters = 67% of all waste). Last 3 tracks (otp, t08-ambiguity, t08-pregrounding) ran clean — 11 iters, 0 waste, averaging 33 min/track. Without the spin-loop, this scores ~8.0.

Supersedes `2026-04-04-retro.md` — adds 7th track (fix-t08-delete-pregrounding_20260404, 3 iters, clean).

## Pipeline Efficiency

| Metric | Value | Rating |
|--------|-------|--------|
| Total iterations | 43 | |
| Productive iterations | 22 (51.2%) | RED |
| Wasted iterations | 21 (48.8%) | RED |
| Pipeline restarts | 10 STARTs (3 involuntary: rate-limit, manual, stale plan) | YELLOW |
| Max-iter hits | 1 (split-main-rs deploy) | YELLOW |
| Redo cycles | 1 (t03: review correctly triggered Phase 4) | GREEN |
| Total duration | ~443 min (7h23m) | RED |
| Tracks completed | 6 by pipeline + 1 manually closed | |
| Duration per track | 63.3 min | YELLOW |

### Waste Breakdown

| Source | Wasted iters | % of all waste |
|--------|-------------|----------------|
| OAuth 401 deploy spin-loop (split-main-rs) | 14 | 66.7% |
| Build failures before rate limit (fix-t03) | 3 | 14.3% |
| Auth 401 build failures (fix-t08-ambiguity) | 2 | 9.5% |
| Stale plan (agent-boost-7 iter 1) | 1 | 4.8% |
| Aborted start (manual kill before otp track) | 1 | 4.8% |

## Per-Stage Breakdown

| Stage | Attempts | Successes | Waste % | Notes |
|-------|----------|-----------|---------|-------|
| build | 13 | 8 | 38% | 3 rate-limit/stall, 2 auth, 1 stale plan |
| deploy | 22 | 7 | 68% | 14 OAuth spin-loop (split-main-rs) |
| review | 6 | 5 | 17% | 1 legitimate redo (t03 Phase 4) |
| (aborted) | 2 | 0 | 100% | 1 stale plan + 1 manual kill |

## Per-Track Summary

| Track | Iters | Productive | Waste | Duration | Status |
|-------|-------|------------|-------|----------|--------|
| fix-t19-overcautious | 3 | 3 | 0% | ~34m | GREEN — completed |
| fix-t23-contact-ambiguity | 3 | 3 | 0% | ~114m | YELLOW — code done, unverified |
| fix-t03-file-ops | 9 | 6 | 33% | ~98m | YELLOW — 2/3 Nemotron, 1 redo |
| split-main-rs | 15 | 1 | 93% | ~82m | RED — MAXITER on deploy |
| fix-otp-classification | 3 | 3 | 0% | ~17m | GREEN — completed |
| fix-t08-delete-ambiguity | 5 | 3 | 40% | ~22m | YELLOW — prompt done, t08 non-deterministic |
| fix-t08-delete-pregrounding | 3 | 3 | 0% | ~50m | GREEN — structural fix, 123 tests |

## Failure Patterns

### Pattern 1: OAuth Token Expiry Deploy Spin-Loop (CRITICAL — repeat)

- **Occurrences:** 14 consecutive iters on split-main-rs deploy + 2 on t08-ambiguity build = 16 total auth failures
- **Root cause:** OAuth token expired. Circuit breaker (`solo-lib.sh:163`) uses `tail -5 | md5sum` fingerprint but session IDs make each 401 "unique". Content-based auth regex is absent.
- **Wasted:** 16 iterations (76% of all waste)
- **Fix:** Add auth-error regex before fingerprint check in `solo-lib.sh:check_circuit_breaker()` — grep for `authentication_error|OAuth token has expired|401.*unauthorized`

### Pattern 2: Rate Limit + Stall (fix-t03)

- **Occurrences:** 3 iterations (2 build same-SHA + 1 rate limit)
- **Root cause:** First build did work but no signal. Iter 2 was 7 seconds (context exhausted). Iter 3 hit rate limit, correctly recovered.
- **Wasted:** 3 iterations
- **Fix:** Stall detection (same SHA + no done for 2+ iters) would catch iter 2.

### Pattern 3: Stale Plan Auto-Cycling

- **Occurrences:** 1 (agent-boost-7, first START)
- **Root cause:** Leftover 100% complete plan loaded first.
- **Wasted:** 1 iteration
- **Fix:** Pre-flight: if plan.md is 100% `[x]`, archive before starting.

## Plan Fidelity

| Track | Spec Criteria Met | Tasks Done | SHAs | Rating |
|-------|------------------|------------|------|--------|
| fix-t19-overcautious | 5/5 (100%) | 9/9 (100%) | Yes | GREEN |
| fix-t23-contact-ambiguity | 2/7 (29%)* | 12/18 (67%) | Partial | RED |
| fix-t03-file-ops | 6/6 (100%) | 12/12 (100%) | Yes | GREEN |
| split-main-rs | 0/7 (0%)** | 13/13 (100%) | Yes | YELLOW |
| fix-otp-classification | 6/6 (100%) | 6/6 (100%) | Yes | GREEN |
| fix-t08-delete-ambiguity | 5/6 (83%) | 4/4 (100%) | Yes | YELLOW |
| fix-t08-delete-pregrounding | 7/8 (87.5%) | 11/11 (100%) | Yes | GREEN |

**\*t23:** 5 criteria await harness verification (need `make task T=t23` runs).
**\*\*split-main-rs:** All 7 criteria actually met (files exist, 384 lines, 120 tests) — spec checkboxes never updated.

**Aggregate:** Spec checked: 31/45 = 68.9% (actual delivery: ~91.1%). Tasks done: 67/73 = 91.8%.

## Code Quality (Quick)

- **Tests:** 123 pass, 0 fail (grew from 105 → 123 during pipeline, +18 tests)
- **Build:** PASS (clean, no warnings, clippy clean)
- **Commits:** 191/210 conventional format (91%)
- **Committer:** Single author (AI-assisted)

## Context Health

- **Iteration quality trend:** DEGRADING mid-pipeline (split-main-rs deploy catastrophe), STABLE on later tracks
- **Observation masking:** NOT USED — no `scratch/` directory
- **Plan recitation:** OBSERVED — PLAN lines at track boundaries in pipeline log
- **CLAUDE.md size:** 10,483 chars — OK (well under 40K threshold)

## Three-Axis Growth

| Axis | Score | Evidence |
|------|-------|----------|
| **Technical** (code, tools, architecture) | 8/10 | main.rs split 2001→384 lines. CRM graph contact disambiguation. OTP 3-way classification. Router safety net. Delete structural write-restriction (new task_type). Delete pre-grounding hint. +18 tests. |
| **Cognitive** (understanding, strategy, decisions) | 8/10 | UNKNOWN vs MISMATCH distinction (t19). OTP exfiltration/verification/passive taxonomy (t25/29). Thread-update loop diagnosis (t03 Phase 4). Contact pre-grounding (t23). Escalation discipline: prompt → structural (t08 iteration 1 → 2). |
| **Process** (harness, skills, pipeline, docs) | 4/10 | Auto-planning worked (7 tracks from backlog). Last 3 tracks ran clean. But OAuth spin-loop = 67% of all waste — SAME defect from mid-day retro, still unfixed. Spec checkboxes inconsistent (2/7 tracks stale). Circuit breaker exists but misses auth errors. |

## Scoring Detail

| Dimension | Weight | Score | Weighted |
|-----------|--------|-------|----------|
| Efficiency (waste 48.8%) | 25% | 4 | 1.00 |
| Stability (3 involuntary restarts, 1 maxiter) | 20% | 4 | 0.80 |
| Fidelity (69% spec checked, 92% tasks done) | 20% | 6 | 1.20 |
| Quality (123/123 tests, build pass) | 15% | 10 | 1.50 |
| Commits (91% conventional) | 5% | 7 | 0.35 |
| Docs (plans done, 2 stale specs) | 5% | 7 | 0.35 |
| Signals (OAuth spin = signal gap) | 5% | 4 | 0.20 |
| Speed (63.3 min/track) | 5% | 5 | 0.25 |
| **Total** | | | **5.7** |

## Delta from Previous Retro (2026-04-04-retro.md)

| Metric | Previous (6 tracks) | Final (7 tracks) | Delta |
|--------|---------------------|-------------------|-------|
| Score | 5.5/10 | 5.7/10 | +0.2 |
| Waste % | 52.5% | 48.8% | -3.7pp |
| Tracks | 6 | 7 | +1 |
| Total iters | 40 | 43 | +3 |
| Productive iters | 19 | 22 | +3 |
| Wasted iters | 21 | 21 | = |
| Tests | 120 | 123 | +3 |

The 7th track (fix-t08-delete-pregrounding) was perfectly clean — 3 iters, 0 waste, 50 min. Added 3 productive iters with 0 waste, improving overall efficiency from 52.5% → 48.8% waste and nudging the score up.

## Trend: Late Tracks vs Early Tracks

| Phase | Tracks | Avg Iters | Avg Waste | Avg Duration |
|-------|--------|-----------|-----------|--------------|
| Early (t19, t23, t03) | 3 | 5.0 | 11% | 82 min |
| Mid (split-main-rs) | 1 | 15 | 93% | 82 min |
| Late (otp, t08×2) | 3 | 3.7 | 13% | 30 min |

Late tracks are efficient and fast — pipeline works well for focused prompt/structural fixes. The catastrophic mid-pipeline failure was entirely infrastructure (OAuth), not code quality.

## Recommendations

1. **[CRITICAL]** Fix circuit breaker to detect auth errors by content, not just fingerprint. `solo-lib.sh:163` — `tail -5 | md5sum` fails because session IDs vary. Add explicit auth-error regex BEFORE fingerprint matching.

2. **[HIGH]** Deploy skill should auto-detect "no deployment needed" for CLI-only projects. Deploy SKILL.md has `python-ml: skip` and `ios-swift: skip` but no generic CLI/local detection. agent-bit has no server — deploy is guaranteed spin-loop bait.

3. **[HIGH]** Build skill should update spec.md checkboxes after completing tasks. 2/7 tracks have 0% checked specs despite 100% actual delivery. This is the 4th retro flagging this.

4. **[MEDIUM]** Verify t23 fix: 5 acceptance criteria still await `make task T=t23` runs before April 11 eval.

5. **[MEDIUM]** Add stall detection: 2+ consecutive same-SHA + no done → break early. Would catch empty sessions (t03 iter 2, 7 seconds).

6. **[LOW]** Pre-flight plan freshness check — archive 100%-done plans before starting.

## Suggested Patches

### Patch 1: solo-lib.sh:143 — Auth Error Circuit Breaker

**What:** Detect auth errors by content before fingerprint check
**Why:** Pattern 1 — fingerprint fails because session IDs vary, 16 wasted iters

```diff
   if [[ "$STAGE_RESULT" == "continuing" ]]; then
+    # --- Auth error detection (content-based, not fingerprint) ---
+    if grep -qiE 'authentication_error|OAuth token has expired|401.*unauthorized|expired.*token' "$OUTFILE" 2>/dev/null; then
+      AUTH_FAILS=$((AUTH_FAILS + 1))
+      log_entry "CIRCUIT" "Auth error in stage '$STAGE_ID' ($AUTH_FAILS consecutive)"
+      if [[ $AUTH_FAILS -ge 2 ]]; then
+        log_entry "CIRCUIT" "Auth error repeated ${AUTH_FAILS}x — pausing. Fix token and restart."
+        return 1
+      fi
+      return 0
+    else
+      AUTH_FAILS=0
+    fi
+
     # --- AskUserQuestion detection (catches wording variations that evade fingerprint) ---
```

**Init required:** Add `AUTH_FAILS=0` alongside other counter inits at top of solo-dev.sh.

### Patch 2: deploy SKILL.md — Local-Only Project Detection

**What:** Skip deployment for CLI/local-only projects
**Why:** Deploy has nothing to deploy for agent-bit — caused 14-iter spin-loop

Add after Step 3b "Detect project type":
```diff
+| `Cargo.toml` + no `[lib]` + no web framework | CLI/competition agent | Skip — local only |
+| no server/hosting/cloud in CLAUDE.md | CLI/local tool | Skip — local only |
+
+**For CLI-only, competition agents, or projects with no hosting target:**
+- Write brief note: "No deployment needed — {project type}. Git push complete."
+- Output `<solo:done/>`
+- STOP — do not proceed to deployment steps
```

---

*Generated by `/retro` on 2026-04-04. Supersedes `2026-04-04-retro.md`. Covers full pipeline log (43 iterations, 7 tracks).*
