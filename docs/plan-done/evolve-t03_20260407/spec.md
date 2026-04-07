# Specification: Stabilize t03

**Track ID:** evolve-t03_20260407
**Type:** Bug
**Created:** 2026-04-07

## Summary

t03: capture-delete with typo. ~60% pass rate on Nemotron. Non-deterministic.
Hint: "inbox capture and distill with a typo"
Has write-nudge counter + capture-delete nudge. Sometimes agent doesn't write/delete in time.

## Acceptance Criteria

- [ ] t03 passes ≥ 80% (4/5 runs)
- [ ] Use `/evolve t03` to iterate
- [ ] No regression on t01, t09
