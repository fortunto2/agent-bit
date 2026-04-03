# Specification: Split main.rs into modules

**Track ID:** split-main-rs_20260403
**Type:** Refactor
**Created:** 2026-04-03
**Status:** Draft

## Summary

`src/main.rs` is 2001 lines — double the 1000-line split threshold. It contains system prompts, dynamic examples, security scanning, inbox classification, domain matching, contact pre-grounding, planning, and agent execution — all mixed with CLI orchestration. This causes high cognitive load for agents working on any single concern (retro flagged "attention dilution risk").

Split into 3 new modules + a lean main.rs (~450 lines). Pure mechanical move — no logic changes, no new features, no refactoring of function internals.

## Acceptance Criteria

- [ ] `src/prompts.rs` exists with `SYSTEM_PROMPT_EXPLICIT`, `PLANNING_PROMPT`, `examples_for_class()`
- [ ] `src/scanner.rs` exists with all security/classification/domain functions (13 functions + 1 struct)
- [ ] `src/pregrounding.rs` exists with contact hints, inbox reading, planning, and agent execution
- [ ] `src/main.rs` is under 500 lines (CLI + orchestration + guess_outcome only)
- [ ] `cargo test` passes — all 113 tests (tests move with their functions)
- [ ] `cargo build` compiles clean (no warnings except existing `#[allow]`)
- [ ] No logic changes — diff is purely `pub(crate)` visibility + `use` imports + file moves

## Dependencies

- None (pure refactor, no new crates)

## Out of Scope

- Refactoring function internals (e.g., shortening run_agent)
- Changing any logic, thresholds, or prompts
- Consolidating duplicate functions (e.g., structural_injection_score exists in both main.rs and classifier.rs — leave as-is)
- Splitting further (scanner.rs ~770 lines is acceptable for now)

## Technical Notes

### Current main.rs structure (2001 lines)

| Lines | Content | Target module |
|-------|---------|---------------|
| 1-66 | Imports, CLI struct, mod declarations | main.rs |
| 69-105 | SYSTEM_PROMPT_EXPLICIT | prompts.rs |
| 112-174 | examples_for_class() | prompts.rs |
| 177-329 | main() | main.rs |
| 335-404 | run_leaderboard, SharedClassifier, run_trial | main.rs |
| 405-452 | auto_submit_if_needed, guess_outcome | main.rs |
| 453-504 | threat_score, prescan_instruction | scanner.rs |
| 505-654 | scan_inbox, analyze_inbox_content | scanner.rs |
| 657-791 | extract_company_ref, structural_injection_score, FileClassification, semantic_classify_inbox_file | scanner.rs |
| 793-1021 | extract_sender_email, extract_sender_domain, domain_stem, collect_account_domains, check_sender_domain_match | scanner.rs |
| 1022-1229 | extract_mentioned_names, resolve_contact_hints, read_inbox_files | pregrounding.rs |
| 1234-1321 | PLANNING_PROMPT, run_planning_phase | pregrounding.rs |
| 1325-1654 | make_llm_config, run_agent | pregrounding.rs |
| 1657-2001 | tests (move with functions) | split across modules |

### Visibility changes needed

Functions currently private in main.rs that cross module boundaries:
- `examples_for_class` → `pub(crate)` in prompts.rs (used by run_agent in pregrounding.rs)
- `prescan_instruction` → `pub(crate)` in scanner.rs (used by run_agent, dry_run in main.rs)
- `scan_inbox` → `pub(crate)` in scanner.rs (used by run_agent in pregrounding.rs)
- `semantic_classify_inbox_file` → `pub(crate)` in scanner.rs (used by run_agent + read_inbox_files in pregrounding.rs)
- `extract_sender_domain` → `pub(crate)` in scanner.rs (used by read_inbox_files + run_agent in pregrounding.rs)
- `check_sender_domain_match` → `pub(crate)` in scanner.rs (used by read_inbox_files in pregrounding.rs)
- `collect_account_domains` → `pub(crate)` in scanner.rs (used by read_inbox_files in pregrounding.rs)
- `analyze_inbox_content` → `pub(crate)` in scanner.rs (used by run_agent in pregrounding.rs)
- `extract_mentioned_names` → `pub(crate)` in pregrounding.rs (used by run_agent)
- `resolve_contact_hints` → `pub(crate)` in pregrounding.rs (used by run_agent)
- `read_inbox_files` → `pub(crate)` in pregrounding.rs (used by run_agent)
- `run_planning_phase` → `pub(crate)` in pregrounding.rs (used by run_agent)
- `make_llm_config` → `pub(crate)` in pregrounding.rs (used by run_agent + run_leaderboard in main.rs)
- `run_agent` → `pub(crate)` in pregrounding.rs (used by run_trial in main.rs)
- `SharedClassifier` type alias → `pub(crate)` in pregrounding.rs or scanner.rs
- `threat_score` → `pub(crate)` in scanner.rs (used by dry_run in main.rs)
- `FileClassification` → `pub(crate)` in scanner.rs
