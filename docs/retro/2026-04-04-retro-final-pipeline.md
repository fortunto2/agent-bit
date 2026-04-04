# Pipeline Retro: agent-bit (2026-04-04, comprehensive)

## Overall Score: 5.6/10

## Pipeline Efficiency

| Metric | Value | Rating |
|--------|-------|--------|
| Total iterations | 70 | |
| Productive iterations | 41 (58.6%) | RED |
| Wasted iterations | 29 (41.4%) | RED |
| Pipeline restarts | 18 START events (6 unexpected) | RED |
| Max-iter hits | 2 (split-main-rs, harden-t23) | YELLOW |
| Rate limits | 3 events | YELLOW |
| Redo cycles | 2 (fix-t03, harden-t03-t08) | GREEN |
| Total duration | ~1279m (21.3h active) | |
| Tracks completed | 12 (+1 aborted, +1 created not started) | |
| Duration per track | ~107m/track | YELLOW |

## Per-Track Breakdown

| Track | Iters | Productive | Waste % | Duration | Notes |
|-------|-------|-----------|---------|----------|-------|
| agent-boost-7 | 1 | 0 | 100% | ~1m | RED Aborted (stale plan) |
| fix-t19-overcautious | 3 | 3 | 0% | 36m | GREEN Clean run |
| fix-t23-contact-ambiguity | 3 | 3 | 0% | 127m | GREEN Long build (CRM graph) |
| fix-t03-file-ops | 8 | 6 | 25% | 98m | YELLOW 1 redo + 2 rate limit |
| split-main-rs | 15 | 1 | 93% | 82m | RED 14-iter deploy spin-loop |
| fix-otp-classification | 3 | 3 | 0% | 22m | GREEN Clean run |
| fix-t08-delete-ambiguity | 5 | 3 | 40% | 28m | YELLOW 2 empty build iters |
| fix-t08-pregrounding | 3 | 3 | 0% | 54m | GREEN Clean run |
| blocking-outcome-validator | 3 | 3 | 0% | 47m | GREEN Clean run |
| harden-t23-nemotron | 4 | 4 | 0% | 385m | RED 329m build + timeout |
| harden-otp + confidence | 6 | 6 | 0% | 143m | GREEN Clean 2-track combined |
| stabilize-decisions | 12 | 3 | 75% | 32m | RED 8 auth error + 1 deploy empty |
| harden-t03-t08 | 7 | 6 | 14% | ~225m | YELLOW 1 redo (caught UTF-8 bug) |

## Failure Patterns

### Pattern 1: Deploy Spin-Loop on Local-Only Project (CRITICAL)
- **Occurrences:** 14 iterations (split-main-rs deploy)
- **Root cause:** agent-bit is a CLI tool with no deployment target. `/deploy` skill doesn't detect local-only projects, can't produce `<solo:done/>` signal, loops until MAXITER.
- **Wasted:** 14 iterations (20% of ALL iterations)
- **Fix:** `/deploy` SKILL.md — pre-check CLAUDE.md for deploy target, emit `<solo:done/>` if CLI/local project

### Pattern 2: Auth Error Spin-Loop (CRITICAL)
- **Occurrences:** 9 iterations (stabilize-decisions: 8 build + 1 deploy)
- **Root cause:** `solo-lib.sh` circuit breaker uses md5 fingerprint on output. Varying session IDs in 401 responses make each failure "unique" to fingerprint matching. No content-based auth error detection.
- **Wasted:** 9 iterations (12.9% of ALL iterations)
- **Fix:** `solo-lib.sh:check_circuit_breaker()` — add `grep -qiE 'authentication_error|OAuth token has expired|401'` BEFORE fingerprint, abort after 2 consecutive matches
- **Note:** Flagged in **8+ previous retros** — still unfixed. Combined with Pattern 1 = 23/29 wasted iters (79% of all waste).

### Pattern 3: Empty Build Iterations (MEDIUM)
- **Occurrences:** 4 iterations (fix-t08-ambiguity: 2, fix-t03 pre-rate: 2)
- **Root cause:** CLI exits with code 130 or near-empty output, treated as rate limit. Agent sometimes produces empty output on first attempt.
- **Wasted:** 4 iterations (5.7% of iterations)
- **Fix:** solo-dev.sh — distinguish exit code 130 (SIGINT) from rate limit. Add 30s backoff before retry.

### Pattern 4: Runaway Build Session (MEDIUM)
- **Occurrences:** 1 (harden-t23-nemotron: 329m single build)
- **Root cause:** Agent launched 24 parallel background tasks in `/build`, no per-iteration timeout. Global 8h timeout eventually caught it.
- **Wasted:** Not wasted (produced valid code), but cost 5.5h on a single iteration.
- **Fix:** solo-dev.sh — per-iteration timeout: 60m build, 30m deploy/review

## Plan Fidelity

| Track | Criteria Met | Tasks Done | SHAs | Rating |
|-------|-------------|------------|------|--------|
| harden-t03-t08 | 90% (9/10) | 100% | yes | YELLOW |
| fix-prompt-regression | 0% (0/8) | 0% (not started) | no | RED |
| fix-t19-overcautious (done) | ~100% | 100% | yes | GREEN |
| fix-t23-contact-ambiguity (done) | ~100% | 100% | yes | GREEN |
| fix-t03-file-ops (done) | ~90% | 100% | yes | GREEN |
| fix-otp-classification (done) | ~100% | 100% | yes | GREEN |
| fix-t08-delete-ambiguity (done) | ~100% | 100% | yes | GREEN |
| fix-t08-pregrounding (done) | ~100% | 100% | yes | GREEN |
| blocking-outcome-validator (done) | ~100% | 100% | yes | GREEN |
| harden-t23-nemotron (done) | ~90% | 100% | yes | YELLOW |
| harden-otp-t25-t29 (done) | ~90% | 100% | yes | YELLOW |
| stabilize-decisions (done) | ~100% | 100% | yes | GREEN |
| confidence-reflection (done) | ~100% | 100% | yes | GREEN |

Average (completed): ~96% criteria, ~100% tasks. Excellent fidelity on finished work.

## Context Health

- Iteration quality trend: STABLE — late tracks as efficient as mid tracks
- Observation masking: NOT USED — no scratch/ directory
- Plan recitation: OBSERVED — agents read plan.md at task boundaries
- CLAUDE.md size: 14,244 chars — OK (under 40K threshold)

## Code Quality (Quick)

- **Tests:** 156 pass, 0 fail
- **Build:** PASS (2 warnings in sgr-agent, non-blocking)
- **Commits:** 257 total, 240 conventional (93.4%)
- **Committer:** 100% fortunto2 (single developer + AI pair)

## Scoring Breakdown

| Dimension | Weight | Score | Weighted |
|-----------|--------|-------|----------|
| Efficiency | 25% | 4 (41.4% waste) | 1.00 |
| Stability | 20% | 3 (6 restarts, 2 maxiter) | 0.60 |
| Fidelity | 20% | 8 (96% criteria on completed) | 1.60 |
| Quality | 15% | 10 (156/156 tests, build green) | 1.50 |
| Commits | 5% | 7 (93.4% conventional) | 0.35 |
| Docs | 5% | 7 (SHAs in all completed plans) | 0.35 |
| Signals | 5% | 4 (deploy spin-loop, auth loops) | 0.20 |
| Speed | 5% | 4 (107m/track, 1-2h range) | 0.20 |
| **Overall** | | | **5.8/10** |

## Three-Axis Growth

| Axis | Score | Evidence |
|------|-------|----------|
| **Technical** | 9/10 | CRM graph, contact disambiguation, OTP hardening, confidence-gated reflection, temperature annealing, OutcomeValidator blocking, delete routing, UTF-8 safe truncation, structural task-type forcing. Test suite 105 -> 156 (+51). 6 clean modules. |
| **Cognitive** | 8/10 | Escalation discipline (suggestive -> directive -> structural) applied consistently. Prompt regression diagnosed correctly (bloat, not code). Weak model constraints understood. Cost policy (Nemotron primary) enforced. |
| **Process** | 3/10 | Auto-planning produced 12 tracks autonomously — impressive throughput. BUT: same 3 factory defects repeated for 8+ retros (auth circuit breaker, deploy local-only, spec checkboxes). Retro -> fix feedback loop completely broken. Reports generated, nothing changes. |

## Recommendations

1. **[CRITICAL]** `solo-lib.sh:check_circuit_breaker()` — add content-based auth error regex BEFORE md5 fingerprint. This single fix would have saved 9 iterations (12.9% of pipeline). Flagged in 8+ retros. The fact this is unfixed proves the retro -> fix loop is broken.

2. **[CRITICAL]** `solo-dev.sh` — add stall detection: track `last_sha`, abort after 3 consecutive same-SHA + no-signal iterations. Would have caught split-main-rs deploy spin-loop (14 iters).

3. **[HIGH]** `/deploy` SKILL.md — add pre-check for local-only/CLI projects: read CLAUDE.md for deploy target, emit `<solo:done/>` if none. agent-bit has nothing to deploy.

4. **[HIGH]** `/build` SKILL.md — add post-phase spec.md checkbox pass. Match completed tasks to acceptance criteria.

5. **[MEDIUM]** `solo-dev.sh` — add per-iteration timeout: 60m build, 30m deploy/review. Would have caught the 329m harden-t23 runaway.

6. **[MEDIUM]** Create retro -> fix tracking. Each retro defect gets a ticket or TODO. Auto-check at start of next pipeline: "are previous retro defects fixed?"

7. **[LOW]** Add `scratch/` convention for observation masking during long pipeline runs.

## Suggested Patches

### Patch 1: solo-dev.sh — Stall detection

**What:** Abort after 3 consecutive same-SHA iterations with no done signal
**Why:** 14-iter deploy spin-loop (Pattern 1) + 8-iter auth loop (Pattern 2)

```diff
+ last_sha=""
+ stall_count=0
  # After capturing iter commit SHA:
+ if [ "$commit_sha" = "$last_sha" ] && [ "$result" != "stage complete" ]; then
+   stall_count=$((stall_count + 1))
+   if [ "$stall_count" -ge 3 ]; then
+     log "STALL" "Same SHA $commit_sha for $stall_count iterations — aborting stage"
+     break
+   fi
+ else
+   stall_count=0
+ fi
+ last_sha="$commit_sha"
```

### Patch 2: solo-lib.sh — Auth error content check

**What:** Detect auth errors by content before fingerprint comparison
**Why:** 9 wasted auth-error iterations (Pattern 2), unfixed for 8+ retros

```diff
  check_circuit_breaker() {
+   # Content-based auth error detection (fast path)
+   if grep -qiE 'authentication_error|OAuth token has expired|401.*unauthorized' "$iter_log" 2>/dev/null; then
+     auth_fail_count=$((auth_fail_count + 1))
+     if [ "$auth_fail_count" -ge 2 ]; then
+       log "CIRCUIT" "Auth error detected $auth_fail_count times — pausing for token refresh"
+       return 1
+     fi
+   else
+     auth_fail_count=0
+   fi
    # Existing fingerprint-based check...
```

---

## Key Insight

**The pipeline produces excellent technical output** (9/10 technical axis, 156 tests, 93% conventional commits, 12 tracks auto-planned and completed) **but wastes 41% of iterations on known infrastructure defects** that have been documented in 8+ retros without a single fix applied.

The bottleneck is NOT the pipeline logic or the skills — the last 7 clean tracks (18 iters, 0 waste) prove the system works. The bottleneck is the **retro -> fix feedback loop**: defects are identified, patches are designed, but nothing is applied to factory scripts.

Until auth circuit breaker + stall detection + local-only deploy detection are fixed in the factory, every pipeline run will carry ~40% waste tax.
