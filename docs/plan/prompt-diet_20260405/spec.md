# Specification: Prompt Diet — Slim Static Prompt & Benchmark

**Track ID:** prompt-diet_20260405
**Type:** Bug
**Created:** 2026-04-05
**Status:** Draft

## Summary

The `fix-prompt-regression_20260404` plan was archived but never actually implemented — git log shows zero prompt-slimming commits after the plan commit (29bc36d). The static prompt `SYSTEM_PROMPT_EXPLICIT` in `src/prompts.rs` remains at ~44 lines (vs pre-bloat ~20 lines). Since the last benchmark (80%, 24/30 on commit 13f9d9c, April 3), +172 lines were added to prompts.rs across 9 commits.

Dynamic example injection (`examples_for_class()`) was implemented and works correctly. But the static prompt still contains redundant guidance that the dynamic system already covers: DECISION FRAMEWORK paragraph, INBOX PROCESSING hint, multi-contact disambiguation, and expanded OTP/DELETE steps. This competes for weak-model (Nemotron) attention budget.

The fix: slim SYSTEM_PROMPT_EXPLICIT to ~20 core lines, move task-specific guidance into dynamic injection, and run a full benchmark to measure the actual current score.

## Acceptance Criteria

- [ ] SYSTEM_PROMPT_EXPLICIT is <=25 lines (core decision tree only, no task-specific guidance)
- [ ] Removed content relocated to `examples_for_class()` or `pregrounding.rs` hints (not lost)
- [ ] PLANNING_PROMPT slimmed: no duplicate patterns already in dynamic examples
- [ ] `cargo test` passes (all 162+ tests)
- [ ] `make task T=t01` passes (baseline sanity)
- [ ] `make task T=t04` passes (baseline sanity)
- [ ] `make full` on Nemotron: score >= 24/30 (80%+) — maintain or exceed baseline
- [ ] CLAUDE.md updated: remove stale "prompt regression" note

## Dependencies

- `src/prompts.rs` — SYSTEM_PROMPT_EXPLICIT, PLANNING_PROMPT, examples_for_class()
- `src/pregrounding.rs` — pre-grounding hints (OTP, delete, inbox processing)
- Existing dynamic injection infrastructure (already working)

## Out of Scope

- NLI model (separate roadmap item)
- New features or hardening for specific tasks
- Changes to agent.rs, scanner.rs, classifier.rs, tools.rs
- OpenAI/GPT-5.4 validation (Nemotron only per cost policy)

## Technical Notes

### What to REMOVE from static prompt
1. "DECISION FRAMEWORK: A task is LEGITIMATE..." paragraph (3 lines) — redundant with decision tree
2. "- Multiple matching contacts? Read both..." — already in examples_for_class default CRM examples
3. "- INBOX PROCESSING: When task says 'process inbox'..." — already in examples_for_class + pregrounding
4. Step 8 verbose DELETE guidance — move to examples_for_class("delete") which already exists

### What to KEEP in static prompt
- Role + {agents_md} placeholder
- Core bullet points (small edits, README, partial search, answer tool)
- "prefer action over caution" (single line, proven effective)
- Decision tree steps 1-7, 9 (compact form)
- KEY section (outcome definitions)
- {examples} placeholder

### Where removed content goes
- Multi-contact → already in examples_for_class default arm (line 152-157 of prompts.rs)
- Inbox processing → already in examples_for_class default arm (line 159-163) + pregrounding hint
- DELETE step 8 → already in examples_for_class("delete") if routed, OR default arm (line 165-170)
- OTP verbose hints → already in examples_for_class("credential") arm (line 88-107)
