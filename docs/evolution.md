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

## 2026-04-04 (Full) | agent-bit | Factory Score: 5/10

Pipeline: build→deploy→review | Tracks: 8 total (t19, t23, t03, split-main-rs, otp, t08×2, blocking-validator) | Iters: 46 | Waste: 45.7%

### Defects
- **CRITICAL** | solo-lib.sh: Auth error circuit breaker STILL missing (**5th retro**). 16 auth-failure iters (76% of all waste). `check_circuit_breaker()` at line 143 has no content-based auth check — fingerprint at line 163 can't catch varying session IDs.
  - Fix: `solo-lib.sh:143` — add `grep -qiE 'authentication_error|OAuth token has expired|401.*unauthorized'` BEFORE fingerprint
- **HIGH** | solo:deploy SKILL.md: No local-only project detection (5th retro). SKILL.md:81 maps `Cargo.toml + [[bin]]` to crates.io — wrong for competition agents.
  - Fix: `/deploy` SKILL.md:91 — add `CLI/competition agent | Skip — local only` row
- **HIGH** | solo:build SKILL.md: Spec checkbox maintenance absent (5th retro). 3/8 tracks had stale spec checkboxes despite 100% delivery.
  - Fix: `/build` SKILL.md — add post-phase spec.md checkbox pass

### Harness Gaps
- **Context:** CLAUDE.md at 10,918 chars — healthy. 6 modules clean. Blocking OutcomeValidator adds depth (classifier.rs: validation + learning lifecycle).
- **Constraints:** Escalation discipline confirmed: t08 prompt→structural, validator 4-phase plan. Clean arch maintained.
- **Precedents:** Last 4 tracks: 14 iters, 0 waste, avg 33 min/track. Pipeline is excellent when not fighting auth. Blocking validator pattern (confidence-gated kNN + retry limit + security exception) is reusable.

### Missing
- Auth error content-based detection in circuit breaker (CRITICAL — unfixed across 5 retros)
- Local-only project detection in deploy skill
- Spec checkbox auto-update in build skill
- Stall detection (same SHA + no signal for 2+ iters)

### What worked well
- Auto-planning: 8 tracks auto-created and executed across full day
- Last 4 tracks: 14 iters, 0 waste — pipeline at its best
- Test suite grew 105 → 131 (+26 tests) with zero regressions
- Blocking OutcomeValidator: clean 4-phase feature delivery (kNN + learning + security exception)
- Plan SHA annotations + spec checkboxes maintained on 5/8 tracks
- Redo limit + rate limit detection both worked correctly
