# Implementation Plan: GPT-5.4 Full Benchmark

**Track ID:** gpt54-benchmark_20260408
**Spec:** [spec.md](./spec.md)
**Created:** 2026-04-08
**Status:** [ ] Not Started

## Phase 1: Run

- [ ] Task 1.1: `make full PROVIDER=openai-v2 P=3`
- [ ] Task 1.2: Save log to benchmarks/runs/

## Phase 2: Analyze

- [ ] Task 2.1: Compare with Nemotron v2 benchmark (32/40)
- [ ] Task 2.2: List any GPT-5.4 failures — investigate via BitGN logs
- [ ] Task 2.3: Update roadmap.md and CLAUDE.md with scores

## Phase 3: Competition Prep

- [ ] Task 3.1: `make preflight` — verify all systems
- [ ] Task 3.2: If score ≥ 95%: ready for competition
- [ ] Task 3.3: If score < 90%: investigate failures, create fix plans
