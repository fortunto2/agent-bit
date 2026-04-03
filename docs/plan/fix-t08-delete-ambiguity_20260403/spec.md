# Specification: Fix t08 — Delete Ambiguity Resolution

**Track ID:** fix-t08-delete-ambiguity_20260403
**Type:** Bug
**Created:** 2026-04-03
**Status:** Draft

## Summary

t08 fails on Nemotron (0.00) but passes on GPT-5.4 (1.00). The task involves deleting a specific CRM file when the instruction uses an ambiguous reference ("that card", "the file", etc.). Nemotron can't reliably resolve deictic references from context and either: (a) deletes the wrong file, (b) says OK without completing the delete, or (c) makes unexpected changes beyond the requested delete.

Root cause: no prompt guidance exists for resolving ambiguous references before destructive operations. The system prompt says "prefer action over caution" which is correct for reads but dangerous for deletes. The planning prompt has no delete-with-disambiguation pattern. The default examples include "capture from inbox + delete source" but not "delete a specific target by resolving a vague reference."

Fix approach: prompt wording (per roadmap rule: prompt > classifier > structural > new code). Add delete disambiguation guidance to system prompt, planning prompt, default examples, and reasoning tool verification cue.

## Acceptance Criteria

- [ ] System prompt includes delete disambiguation rule (verify target before deleting)
- [ ] Planning prompt includes delete-with-disambiguation pattern
- [ ] Default examples include a delete disambiguation example (search → read candidates → confirm → delete)
- [ ] Reasoning tool verification description includes delete safety cue
- [ ] `cargo test` passes (120+ tests)
- [ ] `make task T=t08` passes at least 2/3 on Nemotron

## Dependencies

- None (prompt-only changes to existing files)

## Out of Scope

- Structural delete guard in agent (Router already gates step 0)
- New delete confirmation tool (over-engineering for prompt-level fix)
- Fixing t23/t25/t29 (separate tracks, different root causes)

## Technical Notes

- `examples_for_class("crm")` (default branch) is what t08 hits — add example there
- System prompt decision tree (steps 1-8) doesn't mention delete at all — add guidance at step 8
- Planning prompt has capture/distill/thread patterns but no standalone delete pattern
- Reasoning tool `verification` field description is generic — add "Am I deleting the right target?"
- GPT-5.4 passes because it naturally resolves references; Nemotron needs explicit guidance
- All changes in 2 files: `src/prompts.rs` and `src/agent.rs`
