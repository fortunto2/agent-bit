# Implementation Plan: Split main.rs into modules

**Track ID:** split-main-rs_20260403
**Spec:** [spec.md](./spec.md)
**Created:** 2026-04-03
**Status:** [ ] Not Started

## Overview

Mechanical extraction of main.rs (2001 lines) into 3 modules. No logic changes — only file moves, `pub(crate)` visibility, and `use` imports. Each phase produces a compiling, test-passing codebase.

## Phase 1: Extract prompts.rs <!-- checkpoint:6b14dfd -->

Extract static text (system prompt, planning prompt, examples) into a dedicated module.

### Tasks
- [x] Task 1.1: Create `src/prompts.rs` with `SYSTEM_PROMPT_EXPLICIT`, `PLANNING_PROMPT`, and `examples_for_class()`. Mark all items `pub(crate)`.
- [x] Task 1.2: In `src/main.rs`, remove the moved code and add `mod prompts;`. Update references to use `prompts::SYSTEM_PROMPT_EXPLICIT`, `prompts::PLANNING_PROMPT`, `prompts::examples_for_class`.
- [x] Task 1.3: Run `cargo test` + `cargo build` — verify all 113 tests pass, no new warnings.

### Verification
- [x] `src/prompts.rs` exists with 3 items
- [x] `cargo test` passes
- [x] `cargo build` clean

## Phase 2: Extract scanner.rs <!-- checkpoint:7c35206 -->

Extract security scanning, inbox classification, and domain matching into scanner module (~570 code lines + ~200 test lines).

### Tasks
- [x] Task 2.1: Create `src/scanner.rs` with these functions (all `pub(crate)`)
- [x] Task 2.2: Move corresponding tests from `main.rs::tests` to `scanner.rs` `#[cfg(test)] mod tests`
- [x] Task 2.3: In `src/main.rs`, remove moved code, add `mod scanner;`, update all references to use `scanner::*` paths.
- [x] Task 2.4: Run `cargo test` + `cargo build` — verify all 113 tests pass.

### Verification
- [x] `src/scanner.rs` exists with 13 functions + 1 struct + 31 tests
- [x] `cargo test` passes (113 tests)
- [x] No duplicate function definitions

## Phase 3: Extract pregrounding.rs

Extract contact pre-grounding, inbox reading, planning phase, and agent execution (~630 code lines + ~80 test lines).

### Tasks
- [x] Task 3.1: Create `src/pregrounding.rs` with 6 functions (all `pub(crate)`)
- [x] Task 3.2: Move corresponding tests (4 extract_names + 3 resolve_hints + helper)
- [x] Task 3.3: In `src/main.rs`, remove moved code, add `mod pregrounding;`, update references
- [x] Task 3.4: Run `cargo test` + `cargo build` — verify all 113 tests pass.

### Verification
- [x] `src/pregrounding.rs` exists with 6 functions + 7 tests
- [x] `src/main.rs` is 384 lines (under 500)
- [x] `cargo test` passes (113 tests)

## Phase 4: Docs & Cleanup

### Tasks
- [ ] Task 4.1: Update CLAUDE.md architecture section to reflect new module layout:
  ```
  src/prompts.rs       -- system prompts, planning prompt, dynamic examples
  src/scanner.rs       -- security scanning, inbox classification, domain matching
  src/pregrounding.rs  -- contact pre-grounding, inbox reading, planning, agent execution
  ```
- [ ] Task 4.2: Verify no dead code — `cargo build` with no new warnings. Remove any orphaned imports in main.rs.
- [ ] Task 4.3: Run `make task T=t01` — smoke test that agent still works end-to-end.

### Verification
- [ ] CLAUDE.md reflects new module structure
- [ ] No dead code warnings
- [ ] Agent runs successfully on t01

## Final Verification

- [ ] All acceptance criteria from spec met
- [ ] Tests pass (113)
- [ ] Build clean
- [ ] `src/main.rs` under 500 lines
- [ ] No logic changes (only visibility + imports + file moves)

## Context Handoff

_Summary for /build to load at session start._

### Session Intent

Split `src/main.rs` (2001 lines) into 3 new modules (prompts, scanner, pregrounding) to reduce agent cognitive load. Pure mechanical refactor — no logic changes.

### Key Files

- `src/main.rs` — source of all extractions (currently 2001 lines, target ~450)
- `src/prompts.rs` — NEW: system prompts + examples (~100 lines)
- `src/scanner.rs` — NEW: security + classification + domain matching (~770 lines)
- `src/pregrounding.rs` — NEW: contact hints + inbox + planning + run_agent (~710 lines)
- `CLAUDE.md` — update architecture table

### Decisions Made

- **3 modules, not 4**: retro suggested separate `examples.rs`, but examples_for_class is only 62 lines — merging into prompts.rs is cleaner.
- **SharedClassifier in scanner.rs**: it's a type alias used by scan_inbox and read_inbox_files — scanner owns it, pregrounding imports it.
- **run_agent stays in pregrounding.rs**: it's the biggest consumer of pre-grounding data. main.rs just calls it via run_trial.
- **No function restructuring**: run_agent (305 lines) is large but cohesive — splitting it is a separate track.
- **Tests move with functions**: Rust idiomatic pattern. Each module has its own `#[cfg(test)] mod tests`.

### Risks

- **Cross-module visibility**: many functions are currently private. All need `pub(crate)` — easy but tedious.
- **Import tangles**: run_agent imports from all modules. Verify no circular deps (none expected — deps flow: main → pregrounding → scanner + prompts).
- **Test helpers**: `make_test_crm()` in main.rs tests is used by pregrounding tests — must move with them.

---
_Generated by /plan. Tasks marked [~] in progress and [x] complete by /build._
