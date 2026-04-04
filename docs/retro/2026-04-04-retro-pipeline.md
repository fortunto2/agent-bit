# Pipeline Retro: agent-bit (2026-04-04)

## Overall Score: 5.9/10

Full pipeline session spanning ~24h (Apr 3 16:06 — Apr 4 16:14), processing 10 plan tracks through build->deploy->review stages with auto-plan from backlog.

## Pipeline Efficiency

| Metric | Value | Rating |
|--------|-------|--------|
| Total iterations | 51 | |
| Productive iterations | 31 (60.8%) | YELLOW |
| Wasted iterations | 20 (39.2%) | RED |
| Pipeline restarts | 15 STARTs (10 auto-plan, 3 error recovery, 2 rate limit) | YELLOW |
| Max-iter hits | 2 (split-main-rs deploy, harden-t23 timeout) | RED |
| Rate limit events | 2 | YELLOW |
| Total duration | ~1016m (~17h) | |
| Tracks completed | 10 (9 pipeline, 1 manual recovery) | |
| Duration per track | ~102m/track | YELLOW |

## Per-Stage Breakdown

| Stage | Attempts | Successes | Waste % | Notes |
|-------|----------|-----------|---------|-------|
| build | 22 | 15 | 32% | 5 signal drops, 2 rate limits |
| deploy | 21 | 9 | 57% | 14 wasted on split-main-rs spin-loop |
| review | 8 | 7 | 12% | 1 redo (fix-t03, caught real issues) |

## Failure Patterns

### Pattern 1: Deploy Spin-Loop (split-main-rs)
- **Occurrences:** 14 consecutive iterations (iter 2-15)
- **Root cause:** `/deploy` skill invoked on a pure code-refactoring track (split-main-rs). No server, no deployment target. Deploy couldn't emit `<solo:done/>` because there was nothing to deploy. No circuit breaker — burned all 14 remaining iterations.
- **Wasted:** 14 iterations, ~60m of pipeline time
- **Fix:** `solo-dev.sh` — add stall detection: if same commit SHA + no signal for 3+ consecutive iterations, abort stage. Also: `/deploy` SKILL.md should detect local-only projects and auto-emit `<solo:done/>`.

### Pattern 2: Build Signal Drops
- **Occurrences:** 5 iterations across 3 tracks (agent-boost-7 ×1, fix-t03 ×2, fix-t08-delete ×2)
- **Root cause:** Build skill exits without emitting `<solo:done/>`. Likely causes: rate limit (CLI exit 130), context pressure on long sessions, or build completing work but not signaling.
- **Wasted:** 5 iterations
- **Fix:** Add signal emission reminder earlier in build SKILL.md. Add CLI exit code detection in `solo-dev.sh` (code 130 = rate limit, don't count as real iteration).

### Pattern 3: Global Timeout / Mega-Session
- **Occurrences:** 1 (harden-t23-nemotron build: 329m / 5.5h)
- **Root cause:** Single build session ran for 5.5 hours. Likely: large scope (3-phase plan with 26 tasks), multiple `make task` runs inside one session, no session time limit.
- **Wasted:** 329m consumed before timeout, then required restart
- **Fix:** `solo-dev.sh` — per-iteration timeout (60m for build, 30m for deploy/review). If exceeded, save progress and restart.

### Pattern 4: Rate Limit Disruptions
- **Occurrences:** 2 events (fix-t03 iter 3, harden-t23 restart)
- **Root cause:** CLI exit code 130 with empty output — treated as rate limit by pipeline
- **Wasted:** ~2m recovery each (60s wait + restart)
- **Fix:** Already handled by `solo-dev.sh` rate limit detection. Low impact.

## Per-Track Summary

| Track | Duration | Iters | Wasted | Outcome |
|-------|----------|-------|--------|---------|
| fix-t19-overcautious | 36m | 4 | 1 | Clean |
| fix-t23-contact-ambiguity | 127m | 3 | 0 | Clean (build=114m) |
| fix-t03-file-ops | 98m | 8 | 2 | 1 redo cycle |
| split-main-rs | 82m | 15 | 14 | MAXITER (deploy spin) |
| fix-otp-classification | 22m | 3 | 0 | Clean, fastest track |
| fix-t08-delete-ambiguity | 28m | 5 | 2 | 2 build retries |
| fix-t08-delete-pregrounding | 54m | 3 | 0 | Clean |
| blocking-outcome-validator | 47m | 3 | 0 | Clean |
| harden-t23-nemotron | 385m | 4 | 0 | Timeout + restart, completed |
| harden-otp-t25-t29 | ~137m | 3 | 0 | Clean |

## Plan Fidelity

| Track | Criteria Met | Tasks Done | Rating |
|-------|-------------|------------|--------|
| fix-t19-overcautious | 100% (5/5) | 100% (22/22) | GREEN |
| fix-t23-contact-ambiguity | 29% (2/7) | 44% (16/36) | RED |
| fix-t03-file-ops | 100% (6/6) | 100% (27/27) | GREEN |
| split-main-rs | 0% (0/7) | 100% (33/33) | RED (spec unchecked) |
| fix-otp-classification | 100% (6/6) | 100% (9/9) | GREEN |
| fix-t08-delete-ambiguity | 83% (5/6) | 100% (7/7) | YELLOW |
| fix-t08-delete-pregrounding | 88% (7/8) | 96% (26/27) | YELLOW |
| blocking-outcome-validator | 100% (9/9) | 100% (28/28) | GREEN |
| harden-t23-nemotron | 86% (6/7) | 100% (26/26) | GREEN |
| harden-otp-t25-t29 | 75% (6/8) | 89% (25/28) | YELLOW |
| **Average** | **76%** | **93%** | |

Active plans (not yet completed):
- confidence-reflection: 0% criteria, 3% tasks — barely started
- harden-otp-t25-t29: 75% criteria, 89% tasks — near complete

## Code Quality (Quick)

- **Tests:** 140 pass, 0 fail — GREEN
- **Build:** PASS (cargo build clean) — GREEN
- **Commits:** 240 total, 223 conventional (93%) — GREEN

## Context Health

- **Iteration quality trend:** STABLE — late tracks (blocking-validator, harden-t23, harden-otp) ran cleaner than early tracks
- **Observation masking:** NOT USED — no `scratch/` directory. Long iter logs likely waste context.
- **Plan recitation:** OBSERVED — pipeline reloads plan track at each iteration (PLAN log lines)
- **CLAUDE.md size:** 11,996 chars — OK (well under 40K threshold)

## Scoring Breakdown

| Dimension | Weight | Score | Weighted |
|-----------|--------|-------|----------|
| Efficiency (waste 39%) | 25% | 5 | 1.25 |
| Stability (2 MAXITER, 2 rate limits) | 20% | 3 | 0.60 |
| Fidelity (76% criteria, 93% tasks) | 20% | 7 | 1.40 |
| Quality (140/140 tests) | 15% | 10 | 1.50 |
| Commits (93% conventional) | 5% | 7 | 0.35 |
| Docs (mostly complete) | 5% | 7 | 0.35 |
| Signals (1 major spin-loop) | 5% | 4 | 0.20 |
| Speed (102m/track) | 5% | 4 | 0.20 |
| **Total** | | | **5.9** |

## Three-Axis Growth

| Axis | Score | Evidence |
|------|-------|----------|
| **Technical** | 8/10 | Massive capability expansion: ONNX classifier, OutcomeValidator (adaptive kNN), CRM graph, structural delete routing, OTP classification pipeline, contact pre-grounding. 140 unit tests. main.rs split from 2001→459 lines. |
| **Cognitive** | 7/10 | Evolved from reactive "fix one task" to systematic pattern recognition: domain matching, sender trust, credential exfiltration vs verification distinction. Decision pipeline is well-architected. Missed: fix-t23 criteria 29% suggests scope wasn't fully understood. |
| **Process** | 6/10 | Auto-plan from backlog is excellent automation. Per-track retros good. But: deploy spin-loop burned 14 iters (known from mid-day retro, still not fixed by EOD). No circuit breaker. No observation masking. Two previously-identified factory defects (stall detection, deploy skip) still open. |

## Recommendations

1. **[CRITICAL]** Add stall detection to `solo-dev.sh`: track last commit SHA, if unchanged for 3+ consecutive iterations with no signal, abort stage and log `STALL`. This would have saved 14 iterations on split-main-rs.

2. **[HIGH]** Add per-iteration timeout to `solo-dev.sh`: 60m for build, 30m for deploy/review. The 329m harden-t23 build is extreme — even complex tracks shouldn't need 5.5h in one session.

3. **[HIGH]** Fix `/deploy` skill to detect local-only projects (no server, no hosting config) and auto-emit `<solo:done/>`. Check CLAUDE.md for deployment instructions; if none found, skip gracefully.

4. **[MEDIUM]** `/solo:build` should update spec.md acceptance criteria checkboxes after completing plan tasks. Currently 76% average criteria met vs 93% tasks done — the gap is spec checkbox maintenance, not actual missing work.

5. **[MEDIUM]** Add `scratch/` observation masking for long pipeline runs. Agent should offload large tool outputs to scratch files to preserve context window quality.

6. **[LOW]** CLI exit code 130 should not count as a real iteration attempt. `solo-dev.sh` already handles rate limits but still increments the iteration counter.

## Suggested Patches

### Patch 1: solo-dev.sh — Stall Detection

**What:** Abort stage after 3 consecutive iterations with same commit SHA and no signal
**Why:** Deploy spin-loop on split-main-rs burned 14 of 15 iterations (Pattern 1)

```diff
  # After capturing iter log and checking signal
+ # Stall detection
+ if [ "$current_sha" = "$last_sha" ] && [ "$signal" = "none" ]; then
+   stall_count=$((stall_count + 1))
+   if [ "$stall_count" -ge 3 ]; then
+     log "STALL" "Same SHA ($current_sha) for $stall_count iterations with no signal — aborting stage"
+     break
+   fi
+ else
+   stall_count=0
+ fi
+ last_sha="$current_sha"
```

### Patch 2: deploy SKILL.md — Local-Only Detection

**What:** Auto-complete deploy for projects with no deployment target
**Why:** Pure CLI/library projects have nothing to deploy (Pattern 1)

Add to deploy skill instructions:
```
Before running any deploy steps, check:
1. Does CLAUDE.md mention deployment, hosting, or server?
2. Does the project have Dockerfile, vercel.json, fly.toml, or similar?
If NO to both: emit <solo:done/> with note "No deployment target — local-only project"
```

### Patch 3: build SKILL.md — Spec Checkbox Maintenance

**What:** After completing plan tasks, update corresponding spec.md acceptance criteria
**Why:** 76% criteria met vs 93% tasks done — gap is checkbox maintenance, not missing work
