# Pipeline Retro: agent-bit (2026-04-04) — Comprehensive

## Overall Score: 5.6/10

Full pipeline session: 9 tracks, 48 iterations, 41.7% waste across ~14.5h wall-clock. Strong technical output (1 refactor + 7 bug fixes + 1 feature, 134 tests green, Nemotron 80% baseline maintained). The OAuth deploy spin-loop (14/20 wasted iters = 70% of all waste) remains the singular catastrophe. Last 5 tracks ran perfectly clean — 15 iters, 0 waste, avg 35 min/track. Without the spin-loop, this scores ~8.0.

Supersedes all previous 2026-04-04 retros. Adds 9th track: harden-t23-nemotron (4 iters, 0 waste, but 379m total due to 5.5h build + timeout).

## Pipeline Efficiency

| Metric | Value | Rating |
|--------|-------|--------|
| Total iterations | 48 | |
| Productive iterations | 28 (58.3%) | RED |
| Wasted iterations | 20 (41.7%) | RED |
| Pipeline restarts | 14 STARTs (9 expected plan-cycling, 5 unplanned: 2 rate-limit, 2 timeout, 1 stale) | YELLOW |
| Max-iter hits | 2 (split-main-rs deploy + harden-t23 timeout) | YELLOW |
| Redo cycles | 1 (t03: review correctly triggered Phase 4) | GREEN |
| Total duration | ~873 min (14.5h) | RED |
| Tracks completed | 8 by pipeline + 1 manually closed | |
| Duration per track | 97 min/track | RED |

### Waste Breakdown

| Source | Wasted iters | % of all waste |
|--------|-------------|----------------|
| OAuth 401 deploy spin-loop (split-main-rs) | 14 | 70% |
| Build failures before rate limit (fix-t03) | 2 | 10% |
| Auth 401 build failures (fix-t08-ambiguity) | 2 | 10% |
| Stale plan (agent-boost-7 iter 1) | 1 | 5% |
| Review redo (fix-t03 Phase 4) | 1 | 5% |

## Per-Stage Breakdown

| Stage | Attempts | Successes | Waste % | Notes |
|-------|----------|-----------|---------|-------|
| build | 16 | 11 | 31% | 2 rate-limit, 2 auth, 1 stale plan |
| deploy | 24 | 9 | 63% | 14 OAuth spin-loop (split-main-rs) |
| review | 8 | 7 | 13% | 1 legitimate redo (t03 Phase 4) |

## Failure Patterns

### Pattern 1: OAuth 401 Deploy Spin-Loop (CRITICAL)
- **Occurrences:** 14 consecutive iterations (split-main-rs deploy, lines 160-226 of pipeline.log)
- **Root cause:** `/deploy` invoked on local CLI project (no server). Each iter got OAuth 401, exited with near-empty output (~5 seconds each). Circuit breaker `check_circuit_breaker()` in solo-lib.sh:143 failed because session IDs vary between iterations, making md5 fingerprints unique.
- **Wasted:** 14 iterations (70% of all pipeline waste)
- **Fix:** Two-layer fix needed:
  1. `solo-lib.sh:143` — add auth error regex check before fingerprint: `grep -qiE 'authentication_error|OAuth token has expired|401.*unauthorized'`
  2. `/deploy` SKILL.md — add local-only project detection: if CLAUDE.md has no deploy target, emit `<solo:done/>`

### Pattern 2: Build Rate-Limit Crashes
- **Occurrences:** 2 events (fix-t03 at 19:43:11, harden-t23 at 12:53:52)
- **Root cause:** Claude Code rate limit (exit code 130, empty output). Pipeline correctly detected and retried.
- **Wasted:** 0 real iterations (rate limit handler worked correctly)
- **Fix:** Already handled. No change needed.

### Pattern 3: Short Empty Build Iterations
- **Occurrences:** 4 iterations (fix-t03 iters 1-2, fix-t08-del iters 1-2)
- **Root cause:** Build sessions that exit immediately without producing work. Likely Claude Code session initialization failures or very short context windows.
- **Wasted:** 4 iterations (10% of waste)
- **Fix:** solo-dev.sh — detect iter duration <30 seconds with no signal, don't count against max_iterations

### Pattern 4: Global Timeout on Long Build (harden-t23)
- **Occurrences:** 1 (02:21:08 → 07:50:27, 329 minutes)
- **Root cause:** Build skill ran 24 background tasks (parallel Nemotron test runs). Total session time exceeded 8h global timeout.
- **Impact:** Pipeline stopped, required manual restart next day
- **Wasted:** 0 iterations (the build completed successfully before timeout), but ~5h wall-clock delay
- **Fix:** Background task strategy needs guardrails — cap at 3-5 concurrent background tasks, or use explicit time budgets in build skill

## Plan Fidelity

| Track | Criteria Met | Tasks Done | SHAs | Rating |
|-------|-------------|------------|------|--------|
| fix-t19-overcautious | 5/5 (100%) | 20/20 (100%) | yes | GREEN |
| fix-t23-contact-ambiguity | 2/7 (29%) | 15/35 (43%) | yes | RED |
| fix-t03-file-ops | 6/6 (100%) | 25/25 (100%) | yes | GREEN |
| split-main-rs | stale spec | 31/31 (100%) | yes | YELLOW |
| fix-otp-classification | 6/6 (100%) | 8/8 (100%) | yes | GREEN |
| fix-t08-delete-ambiguity | 5/6 (83%) | 6/6 (100%) | yes | YELLOW |
| fix-t08-delete-pregrounding | 7/8 (88%) | 24/25 (96%) | yes | YELLOW |
| blocking-outcome-validator | 9/9 (100%) | 26/26 (100%) | yes | GREEN |
| harden-t23-nemotron | 6/7 (86%) | all done | yes | GREEN |

**Average:** Criteria 86%, Tasks 93%

**Notes:**
- fix-t23-contact-ambiguity at 29%/43% is the outlier — an ambitious multi-phase plan that was deliberately continued by follow-up tracks (harden-t23-nemotron completed the remaining work)
- split-main-rs spec uses old format (no `[x]` checkboxes), plan is 100%
- harden-t23 has 1 unchecked criterion: GPT-5.4 validation skipped per cost policy (correct decision)
- fix-t08-del/pre: 1-2 unchecked criteria each are non-deterministic test results (acceptable given Nemotron variance)

## Context Health

- **Iteration quality trend:** STABLE — waste concentrated in one catastrophic event (split-main-rs deploy), not gradual degradation. Last 5 tracks = 0 waste.
- **Observation masking:** NOT USED — no `scratch/` directory. agent-bit builds are fast enough that context pressure is rare. Would help during long harden-t23 style builds with many background tasks.
- **Plan recitation:** OBSERVED — build skill correctly loaded plan context at task boundaries
- **CLAUDE.md size:** 11,785 chars — OK (well under 40K threshold)

## Code Quality (Quick)

- **Tests:** 134 pass, 0 fail (grew from 105 at start of pipeline — +29 tests)
- **Build:** PASS (clean, 2 compiler warnings in sgr-agent dep)
- **Commits:** 230 total, 211 conventional format (91.7%)
- **Non-conventional:** 19 commits (mostly old/pre-pipeline)

## Three-Axis Growth

| Axis | Score | Evidence |
|------|-------|----------|
| **Technical** (code, tools, architecture) | 8/10 | main.rs split (2001→384 lines), 6-module architecture, blocking OutcomeValidator (kNN + gated learning), CRM graph with contact disambiguation, 134 tests (+29 new) |
| **Cognitive** (understanding, strategy, decisions) | 8/10 | Nemotron-specific prompt engineering (directive > suggestive, examples > instructions), escalation patterns (prompt → structural), cost-aware testing (Nemotron primary, OpenAI final-only) |
| **Process** (harness, skills, pipeline, docs) | 4/10 | Auth circuit breaker unfixed across 6 retros, deploy skill still spins on local projects, spec checkboxes still manual. Auto-planning and redo limits work well, but known defects accumulate. |

Process axis is the bottleneck. Technical and cognitive growth are strong — the pipeline harness needs to catch up.

## Recommendations

1. **[CRITICAL]** `solo-lib.sh:143` — Add auth error content-based check before fingerprint matching. This defect has been flagged in 6 consecutive retros and accounts for 70% of all pipeline waste. Two lines of grep would eliminate it entirely.

2. **[HIGH]** `/deploy` SKILL.md — Add local-only/CLI project detection. If CLAUDE.md has no deploy target (no hosting, no server), emit `<solo:done/>` immediately. Would prevent the entire split-main-rs catastrophe.

3. **[HIGH]** `/build` SKILL.md — Add post-phase spec.md checkbox update. 3/9 tracks had stale spec checkboxes despite 100% task completion. This has been flagged in 4+ retros.

4. **[MEDIUM]** `solo-dev.sh` — Add short-iter detection. If an iteration completes in <30 seconds with no signal and no commit, don't count against max_iterations. Would prevent 4 wasted iterations from empty build sessions.

5. **[MEDIUM]** Build skill — Cap background tasks at 3-5 concurrent. The harden-t23 build ran 24 background tasks and hit the 8h global timeout. Parallel testing is good but needs guardrails.

6. **[LOW]** `solo-dev.sh` — Add stall detection. If last SHA unchanged for 3+ consecutive iterations, trigger circuit breaker. Catches the deploy spin-loop pattern even without auth-specific detection.

## Suggested Patches

### Patch 1: solo-lib.sh — Auth error circuit breaker

**What:** Add auth error regex check before fingerprint matching
**Why:** 14 wasted iterations on OAuth 401 spin-loop (Pattern 1)

```diff
  check_circuit_breaker() {
+   # Auth error content check (catches varying session IDs)
+   if grep -qiE 'authentication_error|OAuth token has expired|401.*unauthorized' "$iter_log" 2>/dev/null; then
+     log "CIRCUIT" "Auth error detected in iter output — breaking"
+     return 1
+   fi
    local fingerprint=$(tail -20 "$iter_log" | md5sum | cut -d' ' -f1)
```

### Patch 2: deploy SKILL.md — Local-only project detection

**What:** Add pre-check for CLI/competition/local-only projects
**Why:** Deploy stage is meaningless for projects with no hosting target (Pattern 1)

```diff
  ## Stack-to-Platform Mapping
  
  | Stack indicator | Likely platform |
  |----------------|----------------|
+ | Cargo.toml + `[[bin]]` + no server/API | Skip — local CLI project |
+ | CLAUDE.md says "competition agent" / "CLI tool" | Skip — `<solo:done/>` |
  | Cargo.toml + `[[bin]]` | crates.io |
```

### Patch 3: build SKILL.md — Spec checkbox update

**What:** Add post-phase spec.md acceptance criteria checkbox pass
**Why:** 3/9 tracks had stale spec checkboxes despite complete delivery

```diff
  ## After completing a phase
  
  1. Update plan.md — mark completed tasks with `[x]` and SHA annotations
+ 2. Update spec.md — for each acceptance criterion, check if evidence exists (test passing, behavior verified, commit present). Mark `[x]` with verification note.
  3. Commit with `chore(plan): complete Phase N — {summary}`
```

## Factory Critic

### Factory Score: 4/10

The factory's core defect — auth error detection in `solo-lib.sh` — has been identified and documented in **6 consecutive retros** without being fixed. This is now the #1 process debt item.

**Skill quality:**
- `/build`: 7/10 — Executes plans well, but doesn't maintain spec checkboxes
- `/deploy`: 3/10 — No local-only project detection, guaranteed spin-loop on CLI projects
- `/review`: 8/10 — Correctly triggered redo on t03, good quality gate
- Auto-plan: 9/10 — Successfully created 9 tracks from backlog, zero manual intervention needed

**Pipeline reliability:** 4/10 — Circuit breaker exists but is defeated by varying session IDs. Global timeout saved harden-t23 from burning unlimited credits.

**Top factory defects:**
1. `solo-lib.sh:143` `check_circuit_breaker()` — no auth error content check → 14 wasted iters
2. `/deploy` SKILL.md:81 — no local-only detection → guaranteed spin-loop on CLI projects
3. `/build` SKILL.md — no spec checkbox maintenance → stale acceptance criteria

### Harness Evolution

**Context:** CLAUDE.md at 11,785 chars is healthy. 6-module architecture is clean. The blocking OutcomeValidator adds meaningful depth without bloating context.

**Constraints:** Escalation discipline is excellent (t08 prompt→structural, t23 suggestive→directive, validator multi-phase). No architectural violations observed.

**Precedents:**
- Good: Last 5 clean tracks (fix-otp, fix-t08×2, blocking-validator, harden-t23 final) prove the pipeline is excellent when not fighting infrastructure
- Good: Cost policy (Nemotron primary, OpenAI final-only) saved significant credits
- Bad: Auth spin-loop pattern is 100% factory-level — needs factory-level fix
- Bad: Background task explosion (24 tasks) in harden-t23 — needs build skill guardrails

**Decision traces:** The escalation pattern (try prompt fix → if fails, structural fix) works consistently and should be documented as a standard approach in dev-principles.
