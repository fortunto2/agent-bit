# Pipeline Retro: agent-bit (2026-04-04) — Full

## Overall Score: 5.9/10

Full-day pipeline: 8 tracks, 46 iterations, 45.7% waste across ~8h4m. Strong technical output (1 refactor + 6 bug fixes + 1 feature, 131 tests green, Nemotron 80% baseline). The OAuth deploy spin-loop (14/21 wasted iters = 67% of all waste) is the singular catastrophe dragging the score down. Last 4 tracks ran clean — 14 iters, 0 waste, averaging 28 min/track. Without the spin-loop, this scores ~8.2.

Supersedes all previous 2026-04-04 retros. Adds 8th track: blocking-outcome-validator (3 iters, 0 waste, 41 min, +8 tests).

## Pipeline Efficiency

| Metric | Value | Rating |
|--------|-------|--------|
| Total iterations | 46 | |
| Productive iterations | 25 (54.3%) | RED |
| Wasted iterations | 21 (45.7%) | RED |
| Pipeline restarts | 11 STARTs (3 involuntary: rate-limit, manual, stale plan) | YELLOW |
| Max-iter hits | 1 (split-main-rs deploy) | YELLOW |
| Redo cycles | 1 (t03: review correctly triggered Phase 4) | GREEN |
| Total duration | ~484 min (8h4m) | RED |
| Tracks completed | 7 by pipeline + 1 manually closed | |
| Duration per track | 60.5 min | YELLOW |

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
| build | 14 | 9 | 36% | 3 rate-limit/stall, 2 auth, 1 stale plan |
| deploy | 23 | 8 | 65% | 14 OAuth spin-loop (split-main-rs) |
| review | 7 | 6 | 14% | 1 legitimate redo (t03 Phase 4) |
| (aborted) | 2 | 0 | 100% | 1 stale plan + 1 manual kill |

## Per-Track Summary

| Track | Iters | Productive | Waste | Duration | Status |
|-------|-------|------------|-------|----------|--------|
| fix-t19-overcautious | 3 | 3 | 0% | ~34m | GREEN |
| fix-t23-contact-ambiguity | 3 | 3 | 0% | ~114m | YELLOW — unverified |
| fix-t03-file-ops | 9 | 6 | 33% | ~98m | YELLOW — 1 redo |
| split-main-rs | 15 | 1 | 93% | ~82m | RED — MAXITER |
| fix-otp-classification | 3 | 3 | 0% | ~17m | GREEN |
| fix-t08-delete-ambiguity | 5 | 3 | 40% | ~22m | YELLOW |
| fix-t08-delete-pregrounding | 3 | 3 | 0% | ~50m | GREEN |
| **blocking-outcome-validator** | **3** | **3** | **0%** | **~41m** | **GREEN** |

## Failure Patterns

### Pattern 1: OAuth Token Expiry Deploy Spin-Loop (CRITICAL — 5th retro)

- **Occurrences:** 14 consecutive iters on split-main-rs deploy + 2 on t08-ambiguity build = 16 total auth failures
- **Root cause:** OAuth token expired. Circuit breaker (`solo-lib.sh:163`) uses `tail -5 | md5sum` fingerprint but session IDs make each 401 response "unique". Content-based auth regex is absent.
- **Wasted:** 16 iterations (76% of all waste)
- **Status:** UNFIXED after 5 retros. `solo-lib.sh:check_circuit_breaker()` still has no auth-error detection.
- **Fix:** Add `grep -qiE 'authentication_error|OAuth token has expired|401.*unauthorized'` BEFORE fingerprint check at line 143.

### Pattern 2: Rate Limit + Context Exhaustion (fix-t03)

- **Occurrences:** 3 iterations (2 build same-SHA + 1 rate limit)
- **Root cause:** First build did work but no signal. Iter 2 was 7 seconds (context exhausted). Iter 3 hit rate limit.
- **Wasted:** 3 iterations
- **Fix:** Stall detection (same SHA + no signal for 2+ iters) would catch iter 2.

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
| **blocking-outcome-validator** | **9/9 (100%)** | **14/14 (100%)** | **Yes** | **GREEN** |

**\*t23:** 5 criteria await harness verification.
**\*\*split-main-rs:** All 7 criteria met in reality — spec checkboxes never updated.

**Aggregate:** Spec checked: 40/54 = 74.1% (actual delivery: ~93%). Tasks done: 81/87 = 93.1%.

## Code Quality (Quick)

- **Tests:** 131 pass, 0 fail (grew from 105 → 131 during pipeline, +26 tests)
- **Build:** PASS (clean, no warnings, clippy clean)
- **Commits:** 216 total, 197 conventional format (91.2%)
- **Committer:** Single author (AI-assisted)

## Context Health

- **Iteration quality trend:** DEGRADING mid-pipeline (split-main-rs), STABLE on last 4 tracks
- **Observation masking:** NOT USED — no `scratch/` directory
- **Plan recitation:** OBSERVED — PLAN lines at track boundaries in pipeline log
- **CLAUDE.md size:** 10,918 chars — OK (well under 40K threshold)

## Three-Axis Growth

| Axis | Score | Evidence |
|------|-------|----------|
| **Technical** (code, tools, architecture) | 9/10 | main.rs split 2001→384. CRM graph + contact disambiguation. OTP 3-way classification. Delete write-restriction (new task_type). Blocking OutcomeValidator with confidence-gated kNN + score-gated learning. +26 tests (105→131). |
| **Cognitive** (understanding, strategy, decisions) | 8/10 | UNKNOWN vs MISMATCH distinction (t19). OTP exfiltration/verification/passive taxonomy. Thread-update loop diagnosis. Contact pre-grounding. Escalation: prompt → structural (t08). Conservative blocking thresholds + security exception (validator). |
| **Process** (harness, skills, pipeline, docs) | 4/10 | Auto-planning worked (8 tracks from backlog). Last 4 tracks 0% waste. But OAuth spin-loop = 67% of waste — same defect from retro #1, STILL unfixed in solo-lib.sh. Spec checkboxes inconsistent (3/8 tracks stale). Deploy skill still can't detect local-only projects. |

## Scoring Detail

| Dimension | Weight | Score | Weighted |
|-----------|--------|-------|----------|
| Efficiency (waste 45.7%) | 25% | 4 | 1.00 |
| Stability (3 involuntary restarts, 1 maxiter) | 20% | 4 | 0.80 |
| Fidelity (74% spec checked, 93% tasks done) | 20% | 7 | 1.40 |
| Quality (131/131 tests, build pass) | 15% | 10 | 1.50 |
| Commits (91% conventional) | 5% | 7 | 0.35 |
| Docs (plans done, 3 stale specs) | 5% | 8 | 0.40 |
| Signals (OAuth spin = signal gap) | 5% | 4 | 0.20 |
| Speed (60.5 min/track) | 5% | 5 | 0.25 |
| **Total** | | | **5.9** |

## Delta from Previous Retro (2026-04-04-retro-final.md)

| Metric | Previous (7 tracks) | Full (8 tracks) | Delta |
|--------|---------------------|------------------|-------|
| Score | 5.7/10 | 5.9/10 | +0.2 |
| Waste % | 48.8% | 45.7% | -3.1pp |
| Tracks | 7 | 8 | +1 |
| Total iters | 43 | 46 | +3 |
| Productive iters | 22 | 25 | +3 |
| Wasted iters | 21 | 21 | = |
| Tests | 123 | 131 | +8 |

The 8th track (blocking-outcome-validator) was perfectly clean — 3 iters, 0 waste, 41 min, +8 tests. Every new clean track improves the overall efficiency ratio.

## Trend: Track Efficiency Over Time

| Phase | Tracks | Avg Iters | Avg Waste | Avg Duration |
|-------|--------|-----------|-----------|--------------|
| Early (t19, t23, t03) | 3 | 5.0 | 11% | 82 min |
| Mid (split-main-rs) | 1 | 15 | 93% | 82 min |
| Late (otp, t08×2, validator) | 4 | 3.5 | 0% | 33 min |

Late tracks are efficient and fast — pipeline works well for focused fixes. The catastrophic mid-pipeline failure was entirely infrastructure (OAuth), not code quality.

## Recommendations

1. **[CRITICAL]** Fix circuit breaker to detect auth errors by content. `solo-lib.sh:143` — add `grep -qiE 'authentication_error|OAuth token has expired|401.*unauthorized'` BEFORE fingerprint matching. **5th retro flagging this.** 16 wasted iters (76% of all waste) across every pipeline run.

2. **[HIGH]** Deploy skill should auto-detect "no deployment needed" for CLI-only/competition projects. `deploy/SKILL.md:77` detection table has no "local-only → skip" row. agent-bit has no server — deploy is guaranteed spin-loop bait.

3. **[HIGH]** Build skill should update spec.md checkboxes after completing tasks. 3/8 tracks have stale spec checkboxes (0% or <50%) despite 100% actual delivery. 5th retro flagging this.

4. **[MEDIUM]** Add stall detection: 2+ consecutive same-SHA + no done → break early. Would catch context exhaustion (t03 iter 2: 7 seconds, no work done).

5. **[MEDIUM]** Verify t23 fix: 5 acceptance criteria still await `make task T=t23` runs before April 11 eval.

6. **[LOW]** Pre-flight plan freshness check — archive 100%-done plans before starting.

## Suggested Patches

### Patch 1: solo-lib.sh:143 — Auth Error Circuit Breaker

**What:** Detect auth errors by content before fingerprint check
**Why:** Pattern 1 — fingerprint fails because session IDs vary, 16 wasted iters. 5th retro.

```diff
   if [[ "$STAGE_RESULT" == "continuing" ]]; then
+    # --- Auth error detection (content-based) ---
+    if grep -qiE 'authentication_error|OAuth token has expired|401.*unauthorized|expired.*token' "$OUTFILE" 2>/dev/null; then
+      AUTH_FAILS=$((AUTH_FAILS + 1))
+      log_entry "CIRCUIT" "Auth error in stage '$STAGE_ID' ($AUTH_FAILS consecutive)"
+      if [[ $AUTH_FAILS -ge 2 ]]; then
+        log_entry "CIRCUIT" "Auth error repeated ${AUTH_FAILS}x — pausing. Refresh token and restart."
+        return 1
+      fi
+      return 0
+    else
+      AUTH_FAILS=0
+    fi
+
     # --- AskUserQuestion detection (catches wording variations that evade fingerprint) ---
```

**Init required:** Add `AUTH_FAILS=0` alongside other counter inits in solo-dev.sh.

### Patch 2: deploy SKILL.md:81 — Local-Only Project Detection

**What:** Skip deployment for CLI/local-only projects
**Why:** Deploy has nothing to deploy for agent-bit — caused 14-iter spin-loop

```diff
 | `pyproject.toml` (no scripts, no web) | library (Python) | PyPI |
 | `*.xcodeproj` | iOS app | App Store (manual) |
+| `Cargo.toml` + `[[bin]]` + no web/server framework | Competition agent / CLI-only | Skip — local only |
+| No server/hosting/cloud/deploy in CLAUDE.md | Local tool | Skip — local only |
+
+**For local-only, competition agents, or projects with no deploy target:**
+- Write: "No deployment needed — {project type}. Code committed, tests passing."
+- Output `<solo:done/>`
+- STOP
```

### Patch 3: build SKILL.md — Spec Checkbox Auto-Update

**What:** After completing phase tasks, update corresponding spec.md acceptance criteria
**Why:** 3/8 tracks have stale 0% spec checkboxes despite 100% delivery. 5th retro.

Add after the phase completion commit step in build SKILL.md:
```diff
+### Spec Checkbox Pass (after each phase)
+
+After completing all tasks in a phase:
+1. Read `spec.md` acceptance criteria
+2. For each criterion that is now satisfied by completed work:
+   - Change `- [ ]` to `- [x]`
+3. If any checkboxes were updated, include in the phase completion commit
```

---

*Generated by `/retro` on 2026-04-04. Covers full pipeline log (46 iterations, 8 tracks). Supersedes all previous 2026-04-04 retros.*
