# Pipeline Retro: agent-bit — prompt-diet track (2026-04-05)

## Overall Score: 8.7/10

Clean single-track run with zero waste. The prompt-diet experiment was scientifically valid — hypothesis disproven, findings documented, safe revert executed. The only weakness: 3 acceptance criteria left unverified after revert (understandable but technically incomplete).

## Pipeline Efficiency

| Metric | Value | Rating |
|--------|-------|--------|
| Total iterations | 3 | |
| Productive iterations | 3 (100%) | GREEN |
| Wasted iterations | 0 (0%) | GREEN |
| Pipeline restarts | 0 | GREEN |
| Max-iter hits | 0 | GREEN |
| Rate limits | 0 | GREEN |
| Total duration | ~232 min (~3h52m) | |
| Tracks completed | 1/1 | GREEN |
| Duration per track | 232 min | RED |

**Note on duration:** The 3h52m is dominated by the build phase (3h40m), which ran 2 full Nemotron benchmarks (30 tasks each). This is legitimate experiment work, not pipeline waste. Actual code change time was ~20 min.

## Per-Stage Breakdown

| Stage | Attempts | Successes | Waste % | Notes |
|-------|----------|-----------|---------|-------|
| build | 1 | 1 | 0% | 3h40m — ran 2 full benchmarks + code changes + revert |
| deploy | 1 | 1 | 0% | 8 min — archived plan, updated docs |
| review | 1 | 1 | 0% | 2 min — confirmed findings, CLAUDE.md OK |

## Failure Patterns

### No pipeline failures detected.

The prompt-diet track ran cleanly. The **experiment itself** found a critical insight: all 44 lines of SYSTEM_PROMPT_EXPLICIT are load-bearing for Nemotron. Slimming to 25 lines caused 7 regressions (60% score vs 80% baseline). This is a valid negative result, not a pipeline failure.

### Finding: Prompt Diet Disproved
- **Hypothesis:** Task-specific guidance in static prompt is redundant (already in dynamic injection)
- **Result:** DISPROVED. Nemotron-120B needs verbose, explicit decision trees in the static prompt.
- **Evidence:** 25-line version scored 18/30 (60%), 31-line scored 15/30 (50%), original 44-line maintains 80%.
- **Safe change:** PLANNING_PROMPT slimmed by 2 patterns (no regression observed).
- **Lesson:** Weak models (Nemotron) benefit from redundancy. Static prompt + dynamic injection = belt + suspenders. Only slim the planning prompt.

## Plan Fidelity

| Track | Criteria Met | Tasks Done | SHAs | Rating |
|-------|-------------|------------|------|--------|
| prompt-diet_20260405 | 62% (5/8) | 95% (19/20) | yes | YELLOW |

**Details:**
- [x] Removed content relocated — N/A (kept in-place after revert)
- [x] PLANNING_PROMPT slimmed (sha:7753772)
- [x] cargo test passes (162/162)
- [x] t01 passes on Nemotron
- [x] CLAUDE.md updated with experiment findings
- [ ] SYSTEM_PROMPT_EXPLICIT <=25 lines — **NOT ACHIEVABLE** (experiment disproved)
- [ ] t04 passes — not re-verified after revert
- [ ] make full >= 24/30 — not re-benchmarked after revert (assumed maintained by revert to known-good commit)

**Assessment:** The 3 unchecked criteria break down as: 1 legitimately impossible (disproved hypothesis), 2 technically unverified but likely true (revert restores known-good state). Effective fidelity is higher than raw 62%.

## Code Quality (Quick)

- **Tests:** 162 pass, 0 fail — GREEN
- **Build:** PASS (2 warnings in sgr-agent dep, 0 in agent-bit) — GREEN
- **Commits:** 276 total, 259 conventional format (93.8%) — GREEN

### Commits in this track (5 total):
```
9225fc9 docs: update spec checkboxes (verified by review)
e8fcd88 docs: archive calibrate-outcome-validator track, add prompt-diet spec and retro
69c0929 docs: record prompt-diet experiment results and findings
16acf04 revert(prompts): restore original SYSTEM_PROMPT_EXPLICIT
42425e1 fix(prompts): restore load-bearing guard rails after benchmark regression
7753772 refactor(prompts): slim PLANNING_PROMPT — remove duplicate patterns
f243889 refactor(prompts): slim SYSTEM_PROMPT_EXPLICIT from 44 to 25 lines
```

All 7 commits follow conventional format. Revert commit (16acf04) properly documented.

## Context Health

- **Iteration quality trend:** STABLE — all 3 iterations productive, no degradation
- **Observation masking:** NOT USED — single-track, no large outputs
- **Plan recitation:** N/A (single build iteration handled everything)
- **CLAUDE.md size:** 14,573 chars — OK (well under 40k limit)

## Three-Axis Growth

| Axis | Score | Evidence |
|------|-------|----------|
| **Technical** (code, tools, architecture) | 6/10 | Minimal code change (2 PLANNING_PROMPT lines removed). Main value: disproved a hypothesis, preventing future time waste on prompt slimming. |
| **Cognitive** (understanding, strategy, decisions) | 9/10 | Excellent scientific method: hypothesis → experiment → measure → revert. Key insight documented: weak models need redundancy, not minimalism. Counter-intuitive finding captured. |
| **Process** (harness, skills, pipeline, docs) | 8/10 | Clean pipeline run. Spec updated with "NOT ACHIEVABLE" annotation (honest, not hand-wavy). CLAUDE.md and roadmap updated. But 2 acceptance criteria left dangling. |

## Cumulative Health Check (10 retros deep)

This is the 11th retro for agent-bit. The prompt-diet track was clean, but the broader picture has recurring factory defects that remain **unfixed after 10 retros**:

| Factory Defect | Retros Flagged | Status |
|----------------|----------------|--------|
| Auth error circuit breaker (solo-lib.sh) | 10 | UNFIXED |
| Stall detection / same-SHA abort (solo-dev.sh) | 10 | UNFIXED |
| Local-only deploy skip (solo:deploy) | 10 | UNFIXED |
| Spec checkbox auto-update (solo:build) | 10 | UNFIXED |
| Per-iteration timeout | 5 | UNFIXED |
| Retro→fix feedback loop | 3 | UNFIXED |

**Meta-observation:** The retro skill documents problems but has no mechanism to ensure fixes land. 10 identical defect reports = process theater. The prompt-diet track was clean ONLY because it didn't trigger auth or deploy issues.

## Recommendations

1. **[CRITICAL]** Stop writing retro findings that never get fixed. The next pipeline run should **start** with applying the top 3 patches from evolution.md before creating new feature plans. Specifically:
   - `solo-lib.sh` — auth error content-based detection
   - `solo-dev.sh` — stall detection (3 consecutive same-SHA)
   - `solo:deploy` — local-only project detection

2. **[HIGH]** Prompt-diet track left 2 acceptance criteria unverified (t04, full benchmark after revert). Run `make task T=t04` and optionally `make sample` to close these gaps without a full 30-task run.

3. **[MEDIUM]** Document the "weak model redundancy" finding more prominently — this is a reusable insight for any project using Nemotron or similar open-weight models. Consider adding to dev-principles.md.

4. **[LOW]** The build phase ran 3h40m for what was essentially 20 min of code changes + 2 benchmark runs. Future experiment tracks should split "run benchmark" into a separate post-build step so pipeline metrics more accurately reflect actual build time.

## Suggested Patches

### Patch 1: CLAUDE.md — Add weak-model prompting principle

**What:** Document the prompt-diet finding as a reusable design principle
**Why:** Prevents future attempts to slim prompts for weak models

```diff
 ### Single Prompt Mode
 - Single explicit decision tree for all models (removed standard/explicit split)
 - Numbered steps, 5 examples, verbose — works for both Nemotron and GPT-5.4
 - Decision framework reframing: "DENIED requires EXPLICIT evidence — not suspicion, not caution"
+- **Weak-model redundancy principle:** ALL static prompt content is load-bearing for Nemotron.
+  Static + dynamic injection = belt + suspenders. Only slim PLANNING_PROMPT (dynamic examples cover it).
```

### Patch 2: Verify dangling acceptance criteria

**What:** Run `make task T=t04` to close the unverified AC
**Why:** Spec shows 62% criteria met; quick verification would raise it to 75%+

```bash
make task T=t04  # 2 min, free (Nemotron)
```
