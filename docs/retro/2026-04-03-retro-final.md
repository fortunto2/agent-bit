# Pipeline Retro: agent-bit (2026-04-03) — End of Day

## Overall Score: 5.4/10

Five tracks processed across ~6 hours of pipeline runtime. Strong technical output (refactor + 4 bug fixes, all tests green) undermined by a catastrophic deploy spin-loop on `split-main-rs` that burned 14 of 33 total iterations on repeated OAuth 401 errors. Without that spin-loop, this would score ~7.5.

## Pipeline Efficiency

| Metric | Value | Rating |
|--------|-------|--------|
| Total iterations | 33 | |
| Productive iterations | 15 (45.5%) | RED |
| Wasted iterations | 18 (54.5%) | RED |
| Pipeline restarts | 8 START lines (2 true restarts: rate-limit + manual) | YELLOW |
| Max-iter hits | 1 (split-main-rs deploy) | YELLOW |
| Redo cycles | 1 (t03: review correctly triggered Phase 4) | GREEN |
| Total duration | ~360 min (6h) | RED |
| Tracks completed | 5 | |
| Duration per track | ~72 min | YELLOW |

### Waste Breakdown

| Source | Wasted iters | % of all waste |
|--------|-------------|----------------|
| OAuth 401 deploy spin-loop (split-main-rs) | 14 | 78% |
| Build failures before rate limit (fix-t03) | 2 | 11% |
| Stale plan (agent-boost-7 iter 1) | 1 | 5.5% |
| Review redo (fix-t03, legitimate) | 1 | 5.5% |

## Per-Stage Breakdown

| Stage | Attempts | Successes | Waste % | Notes |
|-------|----------|-----------|---------|-------|
| build | 9 | 6 | 33% | 2 from rate-limit, 1 stale plan |
| deploy | 20 | 5 | 75% | 14 OAuth spin-loop iterations |
| review | 4 | 3 | 25% | 1 legitimate redo (t03 Phase 4) |

## Failure Patterns

### Pattern 1: OAuth Token Expiry Deploy Spin-Loop (CRITICAL)

- **Occurrences:** 14 consecutive iterations on split-main-rs deploy stage
- **Consecutive streak:** 14 (iters 2-15, entire remaining budget)
- **Root cause:** OAuth token expired during split-main-rs deploy. The `/deploy` skill invoked `gh` CLI commands which all returned 401. Pipeline retried 14 times with no circuit breaker for authentication failures. Each iteration lasted ~6 seconds (session init + immediate auth failure).
- **Wasted:** 14 iterations + 82 min wall clock (mostly first build iter + spin time)
- **Fix:** Pipeline script needs authentication error detection. If 2+ consecutive iterations fail with the same auth error, break and notify user.

### Pattern 2: Rate Limit Mid-Build

- **Occurrences:** 1 (fix-t03, iteration 3)
- **Root cause:** CLI exit code 130 + near-empty output detected as rate limit. Pipeline correctly waited 60s and restarted.
- **Wasted:** 2 prior iterations (build continuing with no progress) + 1 restart
- **Fix:** Already handled well. Minor improvement: detect "no progress" (same commit SHA across 2+ iterations) as early signal.

### Pattern 3: Stale Plan Loaded First

- **Occurrences:** 1 (agent-boost-7, first START)
- **Root cause:** Leftover plan from previous session loaded before pipeline could cycle to the correct track.
- **Wasted:** 1 iteration
- **Fix:** Auto-plan cleanup before pipeline start, or verify plan freshness.

## Plan Fidelity

| Track | Spec Criteria Met | Tasks Done | SHAs | Rating |
|-------|------------------|------------|------|--------|
| fix-t19-overcautious | 5/5 (100%) | 9/9 (100%) | Yes | GREEN |
| fix-t23-contact-ambiguity | 2/7 (29%) | 12/18 (67%) | Partial | RED |
| fix-t03-file-ops | 6/6 (100%) | 12/12 (100%) | Yes | GREEN |
| split-main-rs | 0/7 (0%)* | 13/13 (100%) | Yes | YELLOW |
| fix-otp-classification | 6/6 (100%) | 6/6 (100%) | Yes | GREEN |

**\*split-main-rs note:** All 7 spec criteria are actually met (files exist, tests pass, main.rs is 384 lines) but spec checkboxes were never updated. Documentation gap only.

**fix-t23 note:** Code is complete (Phases 1-2 done) but 5 harness-verification criteria remain unchecked — blocked by inability to run t23 during pipeline. Phase 3-4 tasks also open.

**Aggregate:** Spec criteria checked: 19/31 = 61% (actual: ~26/31 = 84%). Tasks done: 52/58 = 90%.

## Code Quality (Quick)

- **Tests:** 120 pass, 0 fail
- **Build:** PASS (clean, no warnings)
- **Commits:** 200 total, 181 conventional format (90.5%)
- **Non-conventional prefixes:** `plan:` (2), `security:` (1) — minor, consider `chore:` or `fix:` instead

## Context Health

- **Iteration quality trend:** DEGRADING — stable for first 4 tracks, catastrophic on split-main-rs deploy
- **Observation masking:** NOT USED — no `scratch/` directory exists
- **Plan recitation:** OBSERVED — pipeline logs show PLAN lines at task boundaries
- **CLAUDE.md size:** 9,512 chars — OK (well under 40K threshold)

## Three-Axis Growth

| Axis | Score | Evidence |
|------|-------|----------|
| **Technical** (code, tools, architecture) | 8/10 | main.rs split from 2001 to 384 lines. OTP 3-way classification (exfiltration/verification/passive). CRM graph contact disambiguation. Router safety net prevents permanent tool lockout. |
| **Cognitive** (understanding, strategy, decisions) | 7/10 | Good root cause analysis: UNKNOWN vs MISMATCH distinction (t19), OTP verb analysis (t25/t29), thread-update loop diagnosis (t03 Phase 4). t23 pre-grounding hypothesis still unverified. |
| **Process** (harness, skills, pipeline, docs) | 4/10 | Auto-planning worked well (5 tracks from backlog). But OAuth spin-loop = 42% of all iterations. Spec checkboxes inconsistently maintained. No observation masking. Review redo was legitimate and caught real issues. |

## Scoring Detail

| Dimension | Weight | Score | Weighted |
|-----------|--------|-------|----------|
| Efficiency (waste 54.5%) | 25% | 3 | 0.75 |
| Stability (2 restarts, 1 maxiter) | 20% | 5 | 1.00 |
| Fidelity (61% criteria, 90% tasks) | 20% | 5 | 1.00 |
| Quality (120/120 tests, build pass) | 15% | 10 | 1.50 |
| Commits (90.5% conventional) | 5% | 7 | 0.35 |
| Docs (plans done, some stale specs) | 5% | 7 | 0.35 |
| Signals (OAuth spin = signal gap) | 5% | 4 | 0.20 |
| Speed (72 min/track) | 5% | 5 | 0.25 |
| **Total** | | | **5.4** |

## Recommendations

1. **[CRITICAL]** Add OAuth/auth error circuit breaker to pipeline script. If 2+ consecutive iterations fail with `401` or `authentication_error`, pause pipeline and notify user. The split-main-rs deploy burned 14 iterations (~42% of total waste) on a problem no amount of retrying could fix. **File:** `solo-dev.sh` — add auth-error regex to the rate-limit detection block.

2. **[HIGH]** Add "no progress" detection to pipeline. If 2+ consecutive iterations produce the same commit SHA and no `<solo:done/>` signal, break early. This would have caught both the OAuth spin-loop and the fix-t03 pre-rate-limit build failures. **File:** `solo-dev.sh` — track last commit SHA, increment stall counter.

3. **[HIGH]** Deploy skill should detect "no deployment needed" for CLI/local-only projects. agent-bit has no server/cloud deployment — the deploy stage is either a no-op (CI check) or wasted. Deploy skill should read CLAUDE.md for deploy instructions and emit `<solo:done/>` immediately if project is local-only. **File:** `/deploy` SKILL.md.

4. **[MEDIUM]** Update spec checkboxes as part of `/build` completion, not just `/review`. split-main-rs spec was never updated despite plan being 100% complete. `/build` should check spec criteria after each phase completion.

5. **[MEDIUM]** Verify t23 fix: run `make task T=t23` 3x on Nemotron. Phase 3-4 of fix-t23 plan are still open. The pre-grounding code is deployed but untested against the actual task.

6. **[LOW]** Create `scratch/` directory convention for observation masking. Large tool outputs (file listings, search results) could be offloaded to scratch files to preserve context window quality during long pipeline runs.

## Suggested Patches

### Patch 1: Pipeline — Auth Error Circuit Breaker

**What:** Detect OAuth/auth 401 errors and break after 2 consecutive failures.
**Why:** Pattern 1 above — 14 iterations wasted on unrecoverable auth error.

```diff
# In solo-dev.sh, after iter log capture:
+ # Check for authentication errors
+ if grep -q "authentication_error\|OAuth token has expired\|401" "$iter_log" 2>/dev/null; then
+   AUTH_FAILS=$((AUTH_FAILS + 1))
+   if [ "$AUTH_FAILS" -ge 2 ]; then
+     log "CIRCUIT" "Auth error repeated ${AUTH_FAILS}x — pausing pipeline. Fix token and restart."
+     break
+   fi
+ else
+   AUTH_FAILS=0
+ fi
```

### Patch 2: Pipeline — Stall Detection

**What:** Break if 3+ consecutive iterations produce no new commit and no done signal.
**Why:** Pattern 2 + general safety net for spin-loops.

```diff
# In solo-dev.sh, after commit check:
+ CURRENT_SHA=$(git -C "$PROJECT_ROOT" rev-parse HEAD 2>/dev/null)
+ if [ "$CURRENT_SHA" = "$LAST_SHA" ] && [ "$STAGE_COMPLETE" != "true" ]; then
+   STALL_COUNT=$((STALL_COUNT + 1))
+   if [ "$STALL_COUNT" -ge 3 ]; then
+     log "CIRCUIT" "No progress for ${STALL_COUNT} iterations — breaking."
+     break
+   fi
+ else
+   STALL_COUNT=0
+ fi
+ LAST_SHA="$CURRENT_SHA"
```

---

*Generated by `/retro` on 2026-04-03. Previous mid-day retro: `2026-04-03-retro.md` (covered first 3 tracks only).*
