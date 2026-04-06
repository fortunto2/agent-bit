# Evolution Log — agent-bit

## 2026-04-03 | agent-bit | Factory Score: 6/10

Pipeline: build→deploy→review | Tracks: t19, t23, t03 | Iters: 15 | Waste: 20%

### Defects
- **HIGH** | solo-dev.sh: Stale plan cycling — 100% done plan not auto-archived, wastes 1 iter per occurrence
  - Fix: `solo-dev.sh` — add pre-check: if plan.md is 100% `[x]`, archive before build
- **HIGH** | solo-dev.sh: No OAuth failure detection — auth errors burn iters with retries
  - Fix: `solo-dev.sh` — detect `authentication_error` in iter output, pause for refresh
- **MEDIUM** | solo:build: Doesn't update spec.md acceptance criteria after completing plan tasks
  - Fix: `SKILL.md` for build — add post-completion spec.md checkbox pass

### Harness Gaps
- **Context:** `main.rs` at 2001 lines dilutes agent attention. Prompts, examples, and pre-grounding mixed with orchestration. Future agents editing prompts may miss related code in pre-grounding section.
- **Constraints:** No linter rule for file size. The 1000-line split threshold from dev-principles is manual-only.
- **Precedents:** Write-nudge pattern (3+ consecutive reads → inject nudge) is effective for breaking stuck loops. Worth generalizing to other agent projects.

### Missing
- OAuth token lifecycle management in pipeline scripts
- File size linter (warn on >1000 lines)
- Spec.md auto-checkbox in build skill

### What worked well
- Auto-plan from backlog: pipeline automatically created t23 and t03 plans after t19 completed
- Review redo cycle: correctly identified Phase 4 needed for t03 instead of shipping incomplete
- Rate-limit detection and auto-recovery in solo-dev.sh
- CRM graph architecture: clean separation enabled contact pre-grounding without touching core agent

## 2026-04-03 (EOD) | agent-bit | Factory Score: 5/10

Pipeline: build->deploy->review | Tracks: t19, t23, t03, split-main-rs, fix-otp | Iters: 33 | Waste: 54.5%

### Defects
- **CRITICAL** | solo-dev.sh: No auth error circuit breaker — OAuth 401 burned 14 iterations (42% of all waste) on split-main-rs deploy. Same error repeated 14x with no detection.
  - Fix: `solo-dev.sh` — add auth-error regex check after iter log capture, break after 2 consecutive auth failures
- **HIGH** | solo-dev.sh: No stall detection — if commit SHA unchanged + no done signal for 3+ iterations, pipeline should break
  - Fix: `solo-dev.sh` — track last SHA, increment stall counter, break at 3
- **HIGH** | solo:deploy: Doesn't detect "no deployment needed" for local CLI projects. agent-bit has no server — deploy stage is wasted time or spin-loop bait.
  - Fix: `/deploy` SKILL.md — check CLAUDE.md for deployment instructions, emit `<solo:done/>` if project is local-only
- **MEDIUM** | solo:build: Still doesn't update spec.md checkboxes (repeat from mid-day retro)
  - Fix: `SKILL.md` for build — add spec.md checkbox pass after phase completion

### Harness Gaps
- **Context:** main.rs successfully split (2001 -> 384 lines). Context engineering significantly improved. But no `scratch/` dir for observation masking during long pipeline runs.
- **Constraints:** The split resolved the 2001-line file, but spec checkbox maintenance is still manual. Need automated spec verification.
- **Precedents:** The OAuth spin-loop is a factory-level pattern (failure catalog Pattern 2 + 3 combined). The fix needs to be in solo-dev.sh, not in any project.

### Missing
- Auth error circuit breaker in pipeline script (CRITICAL — biggest single waste source today)
- Stall detection (consecutive same-SHA iterations)
- Local-only project detection in deploy skill
- Observation masking convention (scratch/ directory)

### What worked well
- Auto-planning: 5 tracks auto-generated from backlog across the full day
- main.rs split completed cleanly in 1 build iteration (32 min), tests green
- fix-otp-classification: clean 3-iteration run (build->deploy->review, 17 min total)
- Rate-limit recovery: fix-t03 rate limit handled correctly (60s backoff, successful restart)
- Review redo: correctly caught t03 thread-update loop issue and sent back for Phase 4

## 2026-04-04 | agent-bit | Factory Score: 5/10

Pipeline: build→deploy→review | Tracks: 6 total (t19, t23, t03, split-main-rs, otp, t08) | Iters: 40 | Waste: 52.5%

### Defects
- **CRITICAL** | solo-lib.sh: Fingerprint-based circuit breaker doesn't catch auth errors. Session IDs in 401 output vary between iterations, making each failure "unique" to the md5 fingerprint. 16 auth-failure iterations (76% of all waste) went undetected.
  - Fix: `solo-lib.sh:check_circuit_breaker()` — add content-based auth regex check (grep for `authentication_error|OAuth token has expired|401`) BEFORE fingerprint matching
- **HIGH** | solo:deploy SKILL.md: No detection for local-only projects. agent-bit has no server/hosting — deploy stage is guaranteed spin-loop or no-op. Caused the catastrophic 14-iter spin-loop.
  - Fix: `/deploy` SKILL.md — add pre-check: read CLAUDE.md for deploy target, emit `<solo:done/>` if CLI-only/local project
- **HIGH** | solo:build SKILL.md: Still doesn't update spec.md checkboxes (3rd retro flagging this). 2/6 tracks had 0% spec checkboxes despite 100% task completion.
  - Fix: `/build` SKILL.md — add post-phase step: match completed tasks to spec acceptance criteria checkboxes

### Harness Gaps
- **Context:** CLAUDE.md at 9,512 chars is healthy. main.rs split resolved the 2001-line attention dilution. Module boundaries (prompts.rs, scanner.rs, pregrounding.rs, agent.rs) are clean.
- **Constraints:** No linter enforces module size limits. The split-main-rs refactor was manual (retro-driven). Future growth could re-bloat without automated checks.
- **Precedents:** Late-pipeline tracks (otp, t08) were efficient (3-5 iters, 17-22 min each) — showing that prompt-only fixes execute cleanly when the pipeline isn't fighting infrastructure. The auth spin-loop is 100% a factory defect, not a project issue.

### Missing
- Auth error content-based detection in circuit breaker (CRITICAL — same defect as previous retro, still unfixed)
- Local-only project detection in deploy skill
- Spec checkbox auto-update in build skill
- Stall detection (same SHA + no signal for N iterations)

### What worked well
- Auto-planning from backlog: 6 tracks auto-created and executed across full day
- Circuit breaker concept exists (solo-lib.sh) — just needs content-based augmentation for auth errors
- Redo limit (REDO_MAX=2) — correctly bounded the t03 review→build cycle
- Rate limit detection + exponential backoff — handled correctly on t03
- Plan SHA annotations — good commit traceability in all completed tracks
- Pipeline grew test suite from 105 → 120 tests across the day (net +15 tests)

## 2026-04-04 (Comprehensive) | agent-bit | Factory Score: 4/10

Pipeline: build→deploy→review | Tracks: 9 total (t19, t23, t03, split-main-rs, otp, t08×2, blocking-validator, harden-t23) | Iters: 48 | Waste: 41.7%

### Defects
- **CRITICAL** | solo-lib.sh: Auth error circuit breaker unfixed across **6 retros**. 14 wasted iters (70% of all waste). `check_circuit_breaker()` fingerprint-based — varying session IDs defeat md5 matching.
  - Fix: `solo-lib.sh:143` — add `grep -qiE 'authentication_error|OAuth token has expired|401.*unauthorized'` BEFORE fingerprint
- **HIGH** | solo:deploy SKILL.md: No local-only project detection (6th retro). Caused catastrophic 14-iter spin-loop on split-main-rs.
  - Fix: `/deploy` SKILL.md — add CLI/competition agent detection, emit `<solo:done/>` if no deploy target
- **HIGH** | solo:build SKILL.md: Spec checkbox maintenance absent (6th retro). 3/9 tracks had stale spec checkboxes.
  - Fix: `/build` SKILL.md — add post-phase spec.md checkbox pass
- **MEDIUM** | solo:build: Background task explosion — 24 parallel tasks on harden-t23 caused 5.5h build + global timeout
  - Fix: `/build` SKILL.md — cap concurrent background tasks at 3-5

### Harness Gaps
- **Context:** CLAUDE.md at 11,785 chars — healthy. 6 modules clean. harden-t23 added directive contact disambiguation + inbox processing guidance.
- **Constraints:** Escalation discipline remains strong (suggestive→directive for Nemotron, prompt→structural for t08). Clean arch maintained.
- **Precedents:** Last 5 tracks: 15 iters, 0 waste, avg 35 min/track. Escalation pattern (prompt fix → structural fix) should be documented as standard approach.

### Missing
- Auth error content-based detection in circuit breaker (CRITICAL — 6 retros unfixed)
- Local-only project detection in deploy skill
- Spec checkbox auto-update in build skill
- Background task limits in build skill
- Stall detection (same SHA + no signal for 2+ iters)

### What worked well
- Auto-planning: 9 tracks auto-created and executed across ~14.5h
- Last 5 tracks: 15 iters, 0 waste — pipeline runs clean when not fighting auth
- Test suite grew 105 → 134 (+29 tests) with zero regressions
- Cost policy enforced: Nemotron primary, no unnecessary OpenAI runs
- harden-t23 achieved target (0% → 2/3 on Nemotron) through directive hints + contact pre-loading
- Global timeout correctly caught runaway 5.5h build — prevented unlimited credit burn

## 2026-04-04 (Final) | agent-bit | Factory Score: 4/10

Pipeline: build→deploy→review | Tracks: 10 (full session) | Iters: 51 | Waste: 39.2%

### Defects
- **CRITICAL** | solo-dev.sh: No stall detection. 14-iter deploy spin-loop on split-main-rs burned 27% of all iterations. Same commit SHA repeated 14×, no circuit break.
  - Fix: `solo-dev.sh` — track `last_sha`, increment `stall_count` when SHA unchanged + no signal, abort at 3
- **CRITICAL** | solo-lib.sh: Auth error circuit breaker STILL unfixed across **7 retros**. Fingerprint-based detection defeated by varying session IDs.
  - Fix: `solo-lib.sh` — add content-based `grep -qiE 'authentication_error|401'` before fingerprint matching
- **HIGH** | solo:deploy: No local-only project detection (7th retro). CLI/competition agents have nothing to deploy.
  - Fix: `/deploy` SKILL.md — pre-check CLAUDE.md for deploy target, auto-`<solo:done/>` if none
- **HIGH** | solo:build: Spec checkbox maintenance absent (7th retro). Average 76% criteria met vs 93% tasks done.
  - Fix: `/build` SKILL.md — add post-phase spec.md checkbox pass
- **MEDIUM** | solo:build: No session time limit. harden-t23 build ran 329m (5.5h) in single session.
  - Fix: `solo-dev.sh` — per-iteration timeout: 60m build, 30m deploy/review

### Harness Gaps
- **Context:** CLAUDE.md at 12KB — healthy. 6 clean modules. No scratch/ for observation masking.
- **Constraints:** Pipeline improved: last 6 tracks clean (18 iters, 0 waste). Early tracks carried all the waste.
- **Precedents:** Auto-plan from backlog produced 10 tracks autonomously — impressive throughput. Escalation discipline (prompt→structural) consistently effective.

### Missing
- Stall detection in pipeline script (CRITICAL — would save 14 iters)
- Auth error content-based detection (CRITICAL — 7 retros unfixed)
- Local-only project detection in deploy skill
- Spec checkbox auto-update in build skill
- Per-iteration timeout (session time limit)
- Observation masking (scratch/ convention)

### What worked well
- Auto-planning: 10 tracks auto-created and executed across ~24h
- Last 6 tracks (t08-pregrounding through harden-otp): 18 iters, 0 waste, avg 68m/track
- Test suite grew 105 → 140 (+35 tests) with zero regressions
- Code quality excellent: 93% conventional commits, all 140 tests green
- harden-otp track: OTP classification refined (exfiltration vs verification vs passive)
- CLAUDE.md kept lean (12KB) despite massive feature additions
- Pipeline learned: later tracks dramatically more efficient than early ones

## 2026-04-04 (Session Retro) | agent-bit | Factory Score: 3/10

Pipeline: build→deploy→review | Tracks: 12 (full 17.5h session) | Iters: 65 | Waste: 47.7%

### Defects
- **CRITICAL** | solo-lib.sh: Auth error circuit breaker unfixed across **8+ retros**. 22 wasted iters (34% of total). Fingerprint-based detection defeated by varying session IDs in 401 responses.
  - Fix: `solo-lib.sh:check_circuit_breaker()` — add `grep -qiE 'authentication_error|OAuth token has expired|401'` BEFORE fingerprint, abort after 2 consecutive matches
- **CRITICAL** | solo-dev.sh: No stall detection. 14-iter deploy spin-loop (split-main-rs) + 8-iter auth loop (stabilize-decisions). Same SHA repeated N times with no break.
  - Fix: `solo-dev.sh` — track `last_sha`, abort after 3 consecutive same-SHA + no-signal iterations
- **HIGH** | solo:deploy: No local-only project detection (8th retro). CLI tools have nothing to deploy.
  - Fix: `/deploy` SKILL.md — detect CLI/local in CLAUDE.md, auto-`<solo:done/>`
- **HIGH** | solo:build: Spec checkbox maintenance absent (8th retro). 4/11 tracks had <50% spec checkboxes.
  - Fix: `/build` SKILL.md — add post-phase spec.md checkbox pass
- **MEDIUM** | solo:build: No session time limit. harden-t23 ran 329m before global timeout.
  - Fix: `solo-dev.sh` — per-iteration timeout: 60m build, 30m deploy/review

### Harness Gaps
- **Context:** CLAUDE.md at 13KB — healthy. 6 modules clean. confidence-gated reflection and temperature annealing added. No scratch/ for observation masking.
- **Constraints:** Same 3 factory defects repeated for 8 retros. No feedback loop from retro findings to factory fixes. Retros document problems but nothing changes.
- **Precedents:** Last 6 clean tracks (18 iters, 0 waste) prove the pipeline works when not fighting auth infrastructure. Escalation pattern (suggestive→directive→structural) proven across all task fix tracks.

### Missing
- **Retro → Fix feedback loop (META-CRITICAL):** 8 retros identified the same defects. The retro skill generates reports but has no mechanism to apply patches or track fix status. Need: retro findings → auto-issue creation or patch application.
- Auth error content-based detection (8 retros)
- Stall detection (8 retros)
- Local-only project detection (8 retros)
- Spec checkbox auto-update (8 retros)
- Per-iteration timeout

### What worked well
- Auto-planning: 12 tracks auto-created and executed across 17.5h
- Test suite grew 105 → 147 (+42 tests) with zero regressions
- Code quality: 92% conventional commits, all 147 tests green, build clean
- Technical output excellent: CRM graph, contact disambiguation, OTP hardening, confidence reflection, temperature annealing, outcome validator blocking, delete routing
- CLAUDE.md kept lean (13KB) despite massive feature growth
- Mid-to-late tracks highly efficient — the pipeline is good when auth works

## 2026-04-04 (Final Pipeline) | agent-bit | Factory Score: 3/10

Pipeline: build→deploy→review | Tracks: 13 (12 completed + 1 aborted) | Iters: 70 | Waste: 41.4%

### Defects
- **CRITICAL** | solo-dev.sh: No stall detection — 14-iter deploy spin-loop (split-main-rs) + 9-iter auth loop (stabilize-decisions). 23/29 wasted iters (79% of all waste) from 2 known patterns.
  - Fix: `solo-dev.sh` — track `last_sha`, abort after 3 consecutive same-SHA + no-signal
- **CRITICAL** | solo-lib.sh: Auth error circuit breaker unfixed across **9 retros**. Content-based detection still absent.
  - Fix: `solo-lib.sh:check_circuit_breaker()` — add `grep -qiE 'authentication_error|401'` before fingerprint
- **HIGH** | solo:deploy: No local-only project detection (9th retro).
  - Fix: `/deploy` SKILL.md — pre-check CLAUDE.md, auto-done if no deploy target
- **MEDIUM** | solo:build: No per-iteration timeout. 329m single build (harden-t23).
  - Fix: `solo-dev.sh` — 60m build / 30m deploy-review timeout

### Harness Gaps
- **Context:** CLAUDE.md at 14KB — healthy. 6 modules clean. Prompt regression identified (bloat from bighead additions).
- **Constraints:** Retro→fix feedback loop STILL broken. 9 retros, same 3 factory defects, zero fixes applied. This IS the #1 problem.
- **Precedents:** Escalation discipline proven: suggestive→directive→structural works for all task fixes. Dynamic example injection (pending fix-prompt-regression) should replace static prompt bloat.

### Missing
- **META-CRITICAL:** Retro→fix pipeline. 9 retros documenting same defects = process theater.
- Auth error detection, stall detection, local-only deploy, per-iteration timeout — all designed, none applied.

### What worked well
- Auto-planning: 13 tracks across 21h, only 1 aborted
- Test suite 105→156 (+51 tests), zero regressions, all green
- 93.4% conventional commits (257 total)
- Last 7 tracks: 21 iters, 0 waste — pipeline is excellent when infrastructure works
- Technical output: 9/10 axis. CRM graph, confidence reflection, temperature annealing, outcome validator, delete routing, UTF-8 safe truncation, structural task-type forcing
- Prompt regression correctly diagnosed (spec created, plan ready, not started yet)

## 2026-04-05 | agent-bit | Factory Score: 3/10

Pipeline: build→deploy→review | Tracks: 13 completed, 1 failed (split-main-rs) | Iters: 80 | Waste: 45%

Full cumulative analysis of 2026-04-03 16:06 to 2026-04-05 00:26 marathon session (~20h).

### Defects
- **CRITICAL** | solo-dev.sh: No stall detection — 14-iter deploy spin (split-main-rs) + 8-iter build spin (stabilize-decisions). 22 iters from 2 known patterns. 10th retro flagging this.
  - Fix: `solo-dev.sh` — track last_sha, abort after 3 consecutive same-SHA + no-signal iterations
- **CRITICAL** | solo-dev.sh: Auth error not detected — 3 review iters burned on expired OAuth (calibrate-outcome-validator). Content-based detection still absent. 10th retro flagging this.
  - Fix: `solo-dev.sh` — grep for `authentication_error|401` in iter output, AUTHFAIL halt
- **HIGH** | solo:deploy: No local-only detection (10th retro). agent-bit is a CLI, deploy is meaningless.
  - Fix: `/deploy` SKILL.md — detect CLI/local project, auto-`<solo:done/>`
- **MEDIUM** | solo:build: Spec checkboxes not auto-updated. fix-t23 shows 29% criteria despite archived completion.
  - Fix: `/build` SKILL.md — post-phase spec.md checkbox pass

### Harness Gaps
- **Context:** CLAUDE.md at 14.4KB — well under 40k. 6 modules clean. Calibrate-outcome-validator track added 65 seed examples.
- **Constraints:** **Retro→fix feedback loop remains broken after 10 retros.** Same 3 factory defects documented 10 times, zero patches applied. This is the meta-problem.
- **Precedents:** Pipeline fundamentals are solid — when not fighting auth/stalls, tracks run 3 iters, 0 waste, ~30-50 min each.

### Missing
- **META-CRITICAL:** Retro→fix feedback loop. 10 retros = process theater until patches land.
- Auth error content detection (10 retros)
- Stall detection / circuit breaker (10 retros)
- Local-only deploy skip (10 retros)
- Per-iteration timeout (60m build cap)

### What worked well
- 13 tracks completed autonomously in ~20h — massive throughput
- Test suite: 105→162 (+57 tests), zero regressions
- 93.7% conventional commits (252/269)
- OutcomeValidator calibrated: 65 seeds, adaptive kNN, k=5 blocking
- Last calibrate-outcome-validator track: clean 3-iter finish (pipeline ended strong)
- CLAUDE.md kept lean despite enormous feature additions (14KB)
- Technical axis: 9/10 across all domains (security, ML, CRM, agent architecture)

## 2026-04-05 (prompt-diet) | agent-bit | Factory Score: 8/10

Pipeline: build→deploy→review | Tracks: 1 (prompt-diet) | Iters: 3 | Waste: 0%

### Defects
- None in this track. Clean 3-iteration run.

### Harness Gaps
- **Context:** CLAUDE.md at 14.6KB — healthy. Prompt diet experiment proved all static prompt content load-bearing for Nemotron.
- **Constraints:** Acceptance criteria left partially unverified after revert (t04, full benchmark). Should auto-verify post-revert.
- **Precedents:** **Weak-model redundancy principle discovered:** Nemotron needs verbose static prompt + dynamic injection (belt + suspenders). Slimming from 44→25 lines caused 7 regressions (60% vs 80%). Only PLANNING_PROMPT can be safely slimmed.

### Missing
- Same 4 factory defects from previous 10 retros (auth, stall, deploy-skip, spec-checkbox) — not triggered this track but still unfixed
- Post-revert auto-verification step in /build skill

### What worked well
- Scientific method applied correctly: hypothesis → experiment → benchmark → revert
- Zero pipeline waste — perfect 3/3 iterations
- PLANNING_PROMPT safely slimmed (2 patterns removed, no regression)
- Counter-intuitive finding documented: weak models need redundancy, not minimalism
- Build skill handled complex workflow (code change → benchmark → analyze → revert → re-benchmark → document) in single iteration

## 2026-04-05 (Full Retro) | agent-bit | Factory Score: 3/10

Pipeline: build→deploy→review | Tracks: 17 completed | Iters: ~86 | Waste: 42%

### Defects
- **CRITICAL** | solo-lib.sh: Auth error circuit breaker unfixed across **11 retros**. Fingerprint-based detection defeated by varying session IDs. ~26 iterations lost.
  - Fix: `solo-lib.sh:check_circuit_breaker()` — add `grep -qiE 'authentication_error|OAuth token|401|exit code 130.*empty'` BEFORE fingerprint
- **CRITICAL** | solo-dev.sh: No stall/empty-output detection. 14-iter deploy spin (split-main-rs) + 8-iter build spin (stabilize-decisions).
  - Fix: `solo-dev.sh` — if iter log < 100 bytes AND SHA unchanged, treat as session failure with backoff
- **HIGH** | solo:deploy: No local-only project detection (11th retro). CLI/competition agents = guaranteed spin-loop.
  - Fix: `/deploy` SKILL.md — detect CLI/local from CLAUDE.md, auto-`<solo:done/>`
- **MEDIUM** | solo:build: Spec checkbox maintenance absent (11th retro). fix-t23 at 29% criteria despite completion.
  - Fix: `/build` SKILL.md — post-phase spec.md checkbox pass

### Harness Gaps
- **Context:** CLAUDE.md at 16KB — healthy. 6 modules clean. Outcome verifier added clean 4-file change. No scratch/ for observation masking.
- **Constraints:** **Retro→fix feedback loop broken for 11 retros.** Same defects documented, same patches proposed, zero applied. This file is growing but nothing changes. The retro skill needs a mechanism to track fix status or auto-apply patches.
- **Precedents:** Prompt diet proved ALL static content load-bearing for Nemotron. Outcome verifier pattern (deferred answer → LLM verify → override policy) is clean and generalizable.

### Missing
- **META-CRITICAL:** Retro→fix pipeline. 11 retros = pure documentation theater. Need: retro findings → tracked issues → applied patches → verified fix.
- Auth error content detection (11 retros unfixed)
- Stall detection (11 retros unfixed)
- Local-only deploy skip (11 retros unfixed)
- Per-iteration timeout (60m cap)

### What worked well
- 17 tracks completed autonomously across ~38h wall time — exceptional throughput
- Test suite: 105 → 177 (+72 tests, +69%), zero regressions
- 92% conventional commits (264/286)
- Outcome verifier: clean deferred-answer pattern, 3-file feature in 1 build iteration
- Prompt diet: scientific method applied (hypothesis→experiment→revert), counter-intuitive finding preserved
- Last 4 tracks (calibrate, prompt-diet, outcome-verifier): 12 iters, 0 waste — pipeline excellence when not fighting infrastructure
- Technical axis 9/10: outcome verifier, confidence reflection, temperature annealing, CRM graph, structural task-type forcing, 65-seed adaptive kNN
- Benchmark: Nemotron 80%, GPT-5.4 85% — strong for a free model
