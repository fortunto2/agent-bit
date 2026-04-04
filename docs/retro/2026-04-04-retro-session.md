# Pipeline Retro: agent-bit (2026-04-04, full session)

## Overall Score: 5.3/10

## Pipeline Efficiency

| Metric | Value | Rating |
|--------|-------|--------|
| Total iterations | 65 | |
| Productive iterations | 34 (52.3%) | RED |
| Wasted iterations | 31 (47.7%) | RED |
| Pipeline restarts | 16 START events | RED |
| Max-iter hits | 2 | YELLOW |
| Rate limits | 4 events | YELLOW |
| Redo cycles | 1 (fix-t03) | GREEN |
| Total duration | ~1050m (17.5h) | |
| Tracks completed | 11 | |
| Duration per track | ~95m/track | YELLOW |

## Per-Track Breakdown

| Track | Iters | Productive | Waste % | Duration | Notes |
|-------|-------|-----------|---------|----------|-------|
| agent-boost-7 | 1 | 0 | 100% | ~1m | Aborted (stale plan) |
| fix-t19-overcautious | 3 | 3 | 0% | 36m | GREEN Clean run |
| fix-t23-contact-ambiguity | 3 | 3 | 0% | 127m | GREEN Long build (114m) |
| fix-t03-file-ops | 6 | 5 | 17% | 98m | YELLOW 1 redo cycle, rate limit |
| split-main-rs | 15 | 2 | 87% | 82m | RED 14-iter deploy spin-loop |
| fix-otp-classification | 3 | 3 | 0% | 22m | GREEN Clean run |
| fix-t08-delete-ambiguity | 5 | 3 | 40% | 28m | YELLOW 2 wasted build iters |
| fix-t08-delete-pregrounding | 3 | 3 | 0% | 54m | GREEN Clean run |
| blocking-outcome-validator | 3 | 3 | 0% | 47m | GREEN Clean run |
| harden-t23-nemotron | 2+3 | 1+3 | 20% | 329m+56m | RED Timeout + restart |
| harden-otp + confidence | 3+3 | 6 | 0% | 143m | GREEN Clean combined |
| stabilize-decisions | 12 | 3 | 75% | ~28m | RED 8 auth error iters |

## Failure Patterns

### Pattern 1: OAuth Auth Error Spin-Loop (CRITICAL)
- **Occurrences:** 22 wasted iterations across 2 tracks (split-main-rs: 14, stabilize-decisions: 8)
- **Root cause:** `solo-lib.sh` circuit breaker uses md5 fingerprint on output — but varying session IDs in 401 responses defeat fingerprint matching. Each auth failure looks "unique."
- **Wasted:** 22 iterations (33.8% of ALL pipeline iterations)
- **Fix:** Add content-based auth regex check in `solo-lib.sh:check_circuit_breaker()` BEFORE fingerprint: `grep -qiE 'authentication_error|OAuth token has expired|401.*unauthorized'` → abort after 2 consecutive matches
- **Note:** Flagged in **7+ previous retros** — still unfixed. Single biggest waste source.

### Pattern 2: Deploy Spin-Loop on Local-Only Project
- **Occurrences:** split-main-rs deploy stage: 14 consecutive `continuing` results
- **Root cause:** agent-bit is a CLI tool with no server deployment. `/deploy` skill doesn't detect local-only projects and can't produce `<solo:done/>` signal.
- **Wasted:** 14 iterations (combined with Pattern 1 — same root track)
- **Fix:** `/deploy` SKILL.md — pre-check CLAUDE.md for deploy target, emit `<solo:done/>` immediately if none found
- **Note:** Also flagged in 7+ retros.

### Pattern 3: Spec Checkbox Staleness
- **Occurrences:** 4/11 tracks with <50% spec criteria marked
- **Affected:** confidence-reflection (0/8), split-main-rs (0/7), fix-t23 (2/7)
- **Root cause:** `/build` skill completes plan tasks but doesn't update spec.md acceptance criteria checkboxes
- **Wasted:** 0 iterations directly, but degrades fidelity tracking
- **Fix:** `/build` SKILL.md — add post-phase step: match completed work to spec.md checkboxes

### Pattern 4: Runaway Build Session (329m)
- **Occurrences:** 1 (harden-t23-nemotron)
- **Root cause:** No per-iteration timeout. Build ran 5.5h in a single session (likely background task explosion).
- **Wasted:** Not iterations per se, but 329m of wall time + global timeout trigger
- **Fix:** `solo-dev.sh` — add per-iteration timeout: 60m build, 30m deploy/review

## Plan Fidelity

| Track | Criteria Met | Tasks Done | SHAs | Rating |
|-------|-------------|------------|------|--------|
| fix-t19-overcautious | 100% (5/5) | 100% (20/20) | yes | GREEN |
| fix-t23-contact-ambiguity | 29% (2/7) | 43% (15/35) | yes | RED |
| fix-t03-file-ops | 100% (6/6) | 100% (25/25) | yes | GREEN |
| split-main-rs | 0% (0/7) | 100% (31/31) | yes | RED |
| fix-otp-classification | 100% (6/6) | 100% (8/8) | yes | GREEN |
| fix-t08-delete-ambiguity | 83% (5/6) | 100% (6/6) | yes | YELLOW |
| fix-t08-delete-pregrounding | 88% (7/8) | 96% (24/25) | yes | YELLOW |
| blocking-outcome-validator | 100% (9/9) | 100% (26/26) | yes | GREEN |
| harden-t23-nemotron | 86% (6/7) | 100% (24/24) | yes | YELLOW |
| confidence-reflection | 0% (0/8) | 0% (0/36) | no | RED |
| harden-otp-t25-t29 | 75% (6/8) | 88% (23/26) | yes | YELLOW |
| stabilize-decisions (active) | 89% (8/9) | 80% (20/25) | yes | YELLOW |

**Average spec criteria met:** 69.2% | **Average plan tasks done:** 84.3%

## Code Quality (Quick)

- **Tests:** 147 pass, 0 fail GREEN
- **Build:** PASS GREEN
- **Commits:** 224/244 (91.8%) conventional format GREEN

## Context Health

- Iteration quality trend: DEGRADING (auth errors cluster in later runs; stabilize-decisions burned 8 iters)
- Observation masking: NOT USED (no `scratch/` directory)
- Plan recitation: OBSERVED (pipeline re-reads plan each iteration)
- CLAUDE.md size: 13,177 chars — OK (well under 40K threshold)

## Scoring Breakdown

| Dimension | Weight | Score | Weighted |
|-----------|--------|-------|----------|
| Efficiency | 25% | 4 | 1.00 |
| Stability | 20% | 3 | 0.60 |
| Fidelity | 20% | 5 | 1.00 |
| Quality | 15% | 10 | 1.50 |
| Commits | 5% | 7 | 0.35 |
| Docs | 5% | 7 | 0.35 |
| Signals | 5% | 6 | 0.30 |
| Speed | 5% | 4 | 0.20 |
| **Total** | | | **5.3** |

## Three-Axis Growth

| Axis | Score | Evidence |
|------|-------|----------|
| **Technical** (code, tools, architecture) | 8/10 | 11 tracks shipped: CRM graph, contact disambiguation, OTP classification, confidence-gated reflection, temperature annealing, outcome validator blocking, delete routing. Test suite 105 -> 147 (+42). main.rs split from 2001 to 459 lines across 6 modules. |
| **Cognitive** (understanding, strategy, decisions) | 7/10 | Escalation discipline (suggestive -> directive -> structural). Domain matching evolved from exact-match to stem-based. Credential detection refined (exfiltration vs verification vs passive). Cost policy enforced (Nemotron primary). |
| **Process** (harness, skills, pipeline, docs) | 3/10 | Auth error spin-loop unfixed after 7+ retros. Deploy skill still naive for CLI projects. Spec checkbox maintenance absent. No per-iteration timeout. Same factory defects repeated session after session. |

**Imbalance:** Technical and cognitive growth are strong, but process/harness improvements aren't being fed back to the factory. The pipeline produces good code despite burning ~48% of iterations on infrastructure failures.

## Recommendations

1. **[CRITICAL]** Fix auth error detection in `solo-lib.sh` — content-based regex BEFORE fingerprint matching. Would have saved 22 iterations (34% of total). **This has been flagged for 7+ retros without action.**
2. **[CRITICAL]** Add stall detection in `solo-dev.sh` — track last commit SHA, abort after 3 consecutive same-SHA iterations with no signal.
3. **[HIGH]** Add local-only project detection to `/deploy` SKILL.md — check for deploy target in CLAUDE.md, auto-complete if CLI/local project.
4. **[HIGH]** Add spec.md checkbox update to `/build` SKILL.md post-phase step — match completed tasks to acceptance criteria.
5. **[MEDIUM]** Add per-iteration timeout: 60m build, 30m deploy/review in `solo-dev.sh`.
6. **[LOW]** Create `scratch/` directory convention for observation masking during long pipeline runs.

## Suggested Patches

### Patch 1: solo-lib.sh — Auth error content-based detection

**What:** Add auth error regex check before fingerprint-based circuit breaker
**Why:** 22 wasted iterations (34% of total) from undetected OAuth 401 errors across 2 tracks

```diff
# In check_circuit_breaker() — add before fingerprint check:
+ # Content-based auth error detection (bypasses fingerprint)
+ if grep -qiE 'authentication_error|OAuth token has expired|401.*unauthorized' "$iter_log"; then
+   AUTH_FAIL_COUNT=$((AUTH_FAIL_COUNT + 1))
+   if [ "$AUTH_FAIL_COUNT" -ge 2 ]; then
+     log "CIRCUIT" "Auth error detected ${AUTH_FAIL_COUNT}x — aborting. Refresh OAuth token."
+     return 1
+   fi
+ else
+   AUTH_FAIL_COUNT=0
+ fi
```

### Patch 2: solo-dev.sh — Stall detection

**What:** Track commit SHA across iterations, abort after 3 consecutive stalls
**Why:** 14-iter deploy spin-loop on split-main-rs (same SHA 14 times)

```diff
+ LAST_SHA=""
+ STALL_COUNT=0
# After iter log capture:
+ CURRENT_SHA=$(git -C "$PROJECT_ROOT" rev-parse --short HEAD 2>/dev/null)
+ if [ "$CURRENT_SHA" = "$LAST_SHA" ] && ! grep -q '<solo:done/>' "$iter_log"; then
+   STALL_COUNT=$((STALL_COUNT + 1))
+   if [ "$STALL_COUNT" -ge 3 ]; then
+     log "CIRCUIT" "Stall detected: SHA $CURRENT_SHA unchanged for $STALL_COUNT iters — aborting stage"
+     break
+   fi
+ else
+   STALL_COUNT=0
+ fi
+ LAST_SHA="$CURRENT_SHA"
```

### Patch 3: /deploy SKILL.md — Local-only project detection

**What:** Pre-check for deploy target, auto-complete for CLI/local projects
**Why:** Deploy stage is meaningless for CLI tools — causes spin-loops or no-op waste

```diff
+ ## Pre-Check: Deploy Target
+ Before any deployment work, read the project's CLAUDE.md and check:
+ - If the project mentions "CLI", "local tool", "competition agent", "eval" without any server/hosting:
+   Output: "No deployment target — local-only project. Binary and config ready."
+   Signal: <solo:done/>
```
