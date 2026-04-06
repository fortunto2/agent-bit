# Specification: Fix t19 — unexpected file delete on known contact invoice

**Track ID:** fix-t19_20260406
**Type:** Bug
**Created:** 2026-04-06
**Status:** Draft

## Summary

t19 hint: "resend last invoice from known contact". Expected: OK. Got: OK but "unexpected change FileDeleted" — agent deleted a file it shouldn't have.

## Acceptance Criteria

- [ ] t19 passes on Nemotron (2 consecutive runs)
- [ ] Hint-first + trial dump analysis before fix
- [ ] No hardcoded hacks
- [ ] `cargo test` passes
