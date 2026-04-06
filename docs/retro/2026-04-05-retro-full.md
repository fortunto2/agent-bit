# Pipeline Retro: agent-bit (2026-04-05 Full)

## Overall Score: 5.6/10

38-hour pipeline marathon (2026-04-03 16:06 — 2026-04-05 06:07) completing 17 plan tracks from backlog. Impressive autonomous throughput and feature density. 42% iteration waste from 3 spin-loop incidents and 6 retros worth of unfixed factory defects.

## Pipeline Efficiency

| Metric | Value | Rating |
|--------|-------|--------|
| Total iterations | ~86 | |
| Productive iterations | ~50 (58%) | RED |
| Wasted iterations | ~36 (42%) | RED |
| Pipeline restarts | 22 total (5 unplanned) | RED |
| Max-iter hits | 2 | RED |
| Rate limits | 3 | YELLOW |
| Timeouts | 1 (global 8h) | RED |
| Total duration | ~1500 min (~25h running) | |
| Tracks completed | 17/17 | GREEN |
| Duration per track | ~88 min/track | YELLOW |

## Per-Stage Breakdown

| Stage | Attempts | Successes | Waste % | Notes |
|-------|----------|-----------|---------|-------|
| build | ~38 | 20 | 47% | 8 spin-loop on stabilize-decisions, 2 on fix-t03, 2 on fix-t08 |
| deploy | ~30 | 15 | 50% | 14 spin-loop on split-main-rs (MAXITER), 1 on stabilize |
| review | ~18 | 15 | 17% | 4 spin-loop on calibrate-outcome-validator |

## Failure Patterns

### Pattern 1: Deploy Spin-Loop (split-main-rs)
- **Occurrences:** 14 consecutive deploy failures
- **Root cause:** No local-project detection in `/solo:deploy`. agent-bit is a CLI tool with no server — deploy has nothing to do, never emits `<solo:done/>`
- **Wasted:** 14 iterations (16% of all pipeline iterations)
- **Fix:** `/deploy` SKILL.md — detect CLI/local projects from CLAUDE.md, emit `<solo:done/>` immediately. Flagged in 6+ retros, unfixed.

### Pattern 2: Build Spin-Loop (stabilize-decisions)
- **Occurrences:** 8 consecutive build failures
- **Root cause:** Short-lived sessions (rate limit / CLI exit 130) causing near-empty output. Pipeline treats as "continuing" but no work done.
- **Wasted:** 8 iterations (9% of all iterations)
- **Fix:** `solo-dev.sh` — add empty-output detection: if iter log < 100 bytes AND no commit, trigger backoff instead of retry

### Pattern 3: Review Spin-Loop (calibrate-outcome-validator)
- **Occurrences:** 4 consecutive review failures
- **Root cause:** Same as Pattern 2 — rate limit / session churn causing empty iterations before fingerprint-based circuit breaker can detect repeats
- **Wasted:** 4 iterations
- **Fix:** `solo-lib.sh:check_circuit_breaker()` — add content-based auth regex check BEFORE fingerprint matching

### Pattern 4: Global Timeout (harden-t23-nemotron)
- **Occurrences:** 1
- **Root cause:** Build phase ran 24 parallel background tasks (benchmark runs). 5.5 hours until global 8h timeout killed pipeline.
- **Wasted:** ~329 min of wall time (pipeline recovered on restart)
- **Fix:** `/build` SKILL.md — cap concurrent background tasks at 3-5

### Pattern 5: Redo Cycles (fix-t03, harden-t03-t08)
- **Occurrences:** 2 redo cycles
- **Root cause:** Review correctly found missing work (UTF-8 truncation, capture-delete nudge). Redo was PRODUCTIVE — issues were real.
- **Wasted:** 0 (redo was correct behavior)
- **Impact:** Minor — 2 extra build+deploy iterations each

## Plan Fidelity

| Track | Criteria Met | Tasks Done | Rating |
|-------|-------------|------------|--------|
| fix-t19-overcautious | 100% (5/5) | 100% (20/20) | GREEN |
| fix-t23-contact-ambiguity | 29% (2/7) | 43% (15/35) | RED |
| fix-t03-file-ops | 100% (6/6) | 100% (25/25) | GREEN |
| fix-t08-delete-ambiguity | 83% (5/6) | 100% (6/6) | GREEN |
| fix-t08-pregrounding | 88% (7/8) | 96% (24/25) | GREEN |
| fix-otp-classification | 100% (6/6) | 100% (8/8) | GREEN |
| blocking-outcome-validator | 100% (9/9) | 100% (26/26) | GREEN |
| harden-t23-nemotron | 86% (6/7) | 100% (24/24) | GREEN |
| harden-otp-t25-t29 | 75% (6/8) | 88% (23/26) | YELLOW |
| confidence-reflection | ~100% | ~100% | GREEN |
| stabilize-decisions | 89% (8/9) | 80% (20/25) | YELLOW |
| harden-t03-t08 | 90% (9/10) | 97% (28/29) | GREEN |
| fix-prompt-regression | ~100% | ~100% | GREEN |
| calibrate-outcome-validator | 100% (8/8) | 100% (29/29) | GREEN |
| prompt-diet | 63% (5/8) | 96% (26/27) | YELLOW |
| outcome-verifier | 100% (10/10) | 100% (29/29) | GREEN |

**Average criteria met:** ~87% | **Average tasks done:** ~94%

Notable: fix-t23 at 29% criteria — largest fidelity gap. 5 unchecked acceptance criteria despite pipeline marking as complete.

## Code Quality (Quick)

- **Tests:** 177 pass, 0 fail — GREEN
- **Build:** PASS (cargo build clean)
- **Commits:** 286 total, 264 conventional (92%) — GREEN
- **Test growth:** 105 → 177 (+72 tests, +69%) across pipeline run

## Context Health

- **Iteration quality trend:** DEGRADING — early tracks (t19, t23) ran clean; mid-pipeline spin-loops concentrated on tracks 4-5 and 12-14
- **Observation masking:** NOT USED — no `scratch/` directory. Long build phases (harden-t23: 5.5h) could benefit
- **Plan recitation:** OBSERVED — pipeline re-reads plan at each iteration via PLAN log lines
- **CLAUDE.md size:** 15,968 chars — OK (under 40K threshold)

## Scoring Breakdown

| Dimension | Weight | Score | Calculation |
|-----------|--------|-------|-------------|
| Efficiency | 25% | 4 | 42% waste → score 4 |
| Stability | 20% | 3 | 5 restarts, 2 MAXITER, 1 timeout |
| Fidelity | 20% | 7 | ~87% criteria, ~94% tasks |
| Quality | 15% | 10 | 177/177 tests, build clean |
| Commits | 5% | 7 | 92% conventional |
| Docs | 5% | 7 | All plans archived, most with SHAs |
| Signals | 5% | 4 | 3/17 tracks hit spin-loops |
| Speed | 5% | 4 | 88 min/track |
| **Overall** | | **5.6** | weighted average |

## Three-Axis Growth

| Axis | Score | Evidence |
|------|-------|----------|
| **Technical** | 9/10 | +72 tests. Outcome verifier, confidence reflection, CRM graph, contact disambiguation, temperature annealing, structural task-type forcing, write-nudge, capture-delete nudge. Nemotron 80%, GPT-5.4 85%. |
| **Cognitive** | 8/10 | Prompt diet experiment proved ALL static content is load-bearing. Evolved from prompt tweaks to structural fixes (task-type forcing). Escalation discipline: suggestive → directive → structural. |
| **Process** | 3/10 | Same 3 factory defects flagged across 6+ retros remain unfixed: (1) auth circuit breaker, (2) deploy local detection, (3) spec checkbox auto-update. Evolution log accumulates findings but no one acts on them. |

**Process axis is the bottleneck.** Technical work is excellent — 80% benchmark on a free model is strong. But the pipeline wastes 42% of iterations on known, documented, fixable factory defects.

## Recommendations

1. **[CRITICAL]** Fix auth circuit breaker: `solo-lib.sh:check_circuit_breaker()` — add `grep -qiE 'authentication_error|OAuth token|401.*unauthorized|exit code 130.*empty'` BEFORE fingerprint matching. This single fix would have saved ~26 iterations (30% of total).

2. **[CRITICAL]** Add local-project detection to `/deploy`: read CLAUDE.md for deploy target. If CLI-only / competition agent / no server → emit `<solo:done/>` immediately. Would have saved 14 iterations on split-main-rs.

3. **[HIGH]** Add empty-output detection to `solo-dev.sh`: if iter log < 100 bytes AND commit SHA unchanged, treat as rate-limit/session failure with exponential backoff — not as "continuing".

4. **[HIGH]** Cap background tasks in `/build` SKILL.md to max 3-5 concurrent. The 24-parallel-task build on harden-t23 caused a 5.5h timeout.

5. **[MEDIUM]** Auto-update spec.md checkboxes in `/build` SKILL.md after phase completion. fix-t23 shows 29% criteria met despite task completion — spec checkboxes were never ticked.

6. **[LOW]** Add `scratch/` directory convention for observation masking during long pipeline runs.

## Suggested Patches

### Patch 1: solo-lib.sh — Auth error content-based detection

**What:** Add content-based auth check before fingerprint-based circuit breaker
**Why:** Fingerprint matching fails because session IDs vary between 401 responses

```diff
 check_circuit_breaker() {
+    # Content-based auth error detection (catches varying session IDs)
+    if grep -qiE 'authentication_error|OAuth token has expired|401.*unauthorized' "$iter_log" 2>/dev/null; then
+        log "CIRCUIT" "Auth error detected in iter output — pausing for token refresh"
+        return 1
+    fi
+    if [[ $(wc -c < "$iter_log") -lt 100 ]] && [[ "$current_sha" == "$last_sha" ]]; then
+        log "CIRCUIT" "Empty output + no commit — treating as session failure"
+        return 1
+    fi
     local fingerprint=$(tail -20 "$iter_log" | md5)
```

### Patch 2: deploy SKILL.md — Local project detection

**What:** Skip deploy for CLI-only projects
**Why:** 14 iterations wasted on split-main-rs deploy spin-loop

```diff
 ## Pre-checks
+
+Before deploying, check CLAUDE.md and project structure:
+- If project is a CLI tool, competition agent, or has no server/hosting config → emit `<solo:done/>` with note "local-only project, no deployment needed"
+- Look for: Dockerfile, fly.toml, vercel.json, wrangler.toml, netlify.toml, railway.toml
+- If CLAUDE.md mentions "CLI", "competition", "benchmark agent" and has NO deploy section → skip
```
