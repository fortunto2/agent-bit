# Development Workflow — agent-bit

## Stack
- Rust (edition 2024) + tokio async
- sgr-agent (LLM framework) + bitgn-sdk (Connect-RPC client)
- ONNX Runtime (ML classifier) + ammonia (HTML sanitizer)

## Build & Test
```bash
cargo build                        # compile
cargo test                         # 113 unit tests — MUST pass before commit
make task T=tXX                    # verify specific PAC1 task (default: nemotron)
make task T=tXX PROVIDER=openai    # verify on GPT-5.4
make sample                        # quick 8-task smoke test
make full P=3                      # full 30-task benchmark
```

## TDD Policy
- **Moderate TDD**: write tests for new detection logic (threat_score, domain matching, structural signals)
- Skip TDD for prompt wording changes — verify with `make task` instead
- Every code change: `cargo test` + at least 1 task smoke test

## Commit Strategy
- Conventional commits: `feat:`, `fix:`, `refactor:`, `docs:`, `chore:`
- Include task IDs: `fix(t19): separate MISMATCH from UNKNOWN in ensemble`
- Commit after each phase, not each task
- `Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>`

## Deploy
- **No deploy stage.** This is a CLI agent, not a web service.
- Ship = commit. Private GitHub repo, no CI/CD.
- Competition: `cargo run -- --provider nemotron --parallel 3`

## Review Principles

### NO hardcoded hacks
Competition tasks are randomized and will change. Every fix must be **universal**:
- NO task-ID checks (`if task == "t19"`)
- NO keyword lists for specific tasks
- Prefer: ML classifier tuning > prompt wording > structural signals > new code
- Ask: "Would this fix work if the task wording changed?" If no → it's a hack.

### Intervention hierarchy (prefer higher)
1. **Prompt wording** — rephrase decision tree, add example
2. **Classification recommendation** — change `semantic_classify_inbox_file()` output
3. **Ensemble weights/thresholds** — adjust ML/structural balance
4. **Sender trust logic** — CRM graph domain matching
5. **Structural signal** — new pattern in `structural_injection_score()`
6. **Tool description** — enrich tool schema descriptions
7. **Answer validation** — `validate_answer()` keyword/embedding checks

### Quality gate
Before marking any plan complete:
1. `cargo test` — all pass
2. `make task T=<target>` — target task passes
3. `make task T=t01` — regression check on baseline task
4. No new compiler warnings from our code

## Plan Structure
Plans in `docs/plan/{trackId}/` (spec.md + plan.md).
Completed plans archived to `docs/plan-done/`.
Roadmap: `docs/roadmap.md`.

## Skills
- `/evolve tXX` — autonomous hypothesis-test loop for one task
- `/solo:plan "description"` — create spec + plan
- `/solo:build trackId` — execute plan with TDD
- `/solo:review` — final quality gate
