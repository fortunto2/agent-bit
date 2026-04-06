# Specification: Competition Benchmark & Stabilize

**Track ID:** competition-benchmark_20260406
**Type:** Chore
**Created:** 2026-04-06
**Status:** Draft

## Summary

Last full benchmark (1218845, Apr 5) showed 21/40 = 52.5%, but ~45 commits of improvements have landed since: NLI 3-way ensemble, account context pre-loading (accounts_summary + account_manager + annotate_account_results), scanner recommendation threading, Signal 5 narrowed to domain-mismatch-only, expand_query swapped-name support, and dead code cleanup. The score data is stale and unreliable.

Five new tasks (t35, t36, t38-40) failed on the only benchmark run. All five involve **account paraphrases** — the exact problem account-context_20260406 was designed to solve. These may already be fixed but have never been verified.

Competition is April 11 (5 days). This track establishes the real baseline, diagnoses remaining failures, and applies targeted fixes to maximize first-run success rate.

## Acceptance Criteria

- [ ] Full Nemotron benchmark run on current code (all 40 tasks), results recorded in `benchmarks/runs/`
- [ ] Per-task score table with failure categorization: regression vs non-deterministic vs new-failure
- [ ] Each failing task diagnosed: hint read, score_detail analyzed, trial logs reviewed
- [ ] Top 3 highest-impact failures have targeted fixes applied (code changes)
- [ ] Re-benchmark confirms fixes work (targeted tasks, not full re-run needed)
- [ ] `cargo test` passes after all changes
- [ ] Roadmap updated with current score and remaining failure analysis
- [ ] Runbook updated with current non-deterministic task list and pass rates

## Dependencies

- BitGN harness must be reachable (CF Workers AI Gateway)
- `CF_AI_API_KEY` env var set
- ONNX models present in `models/`

## Out of Scope

- GPT-5.4 benchmark (costs money — reserve for competition day)
- Solo-factory pipeline fixes (circuit breaker, timeouts — tracked in retro, not agent-bit code)
- New ONNX model training or hypothesis generation
- Architectural refactors

## Technical Notes

- Account paraphrase tasks (t35, t38-40) depend on `accounts_summary()` in `crm_graph.rs:466` being injected at `pregrounding.rs:432`
- Swapped-name task (t40) depends on `expand_query()` in `tools.rs:314` which reverses 2-word queries
- t36 may be DENIED over-caution — check if security signal refinement (Signal 5 narrowing) resolved it
- Nemotron is non-deterministic (±4 tasks per run). Single benchmark gives approximate signal, not exact.
- `make task T=tXX` uses evolve skill's run-task.sh which outputs score_detail + trial logs
- The mandatory debugging workflow: hint → score_detail → trial logs → hypothesis → fix
