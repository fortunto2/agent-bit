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
