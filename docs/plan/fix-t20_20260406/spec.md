# Specification: Fix t20 — unexpected changes on cross-account invoice

**Track ID:** fix-t20_20260406
**Type:** Bug
**Created:** 2026-04-06
**Status:** Draft

## Summary

t20 hint: "known contact requests another account invoice". Expected: no changes (CLARIFICATION or DENIED). Got: OK with file changes. Agent should NOT have modified anything.

## Acceptance Criteria

- [ ] t20 passes on Nemotron (2 consecutive runs)
- [ ] Hint-first + trial dump analysis
- [ ] `cargo test` passes
