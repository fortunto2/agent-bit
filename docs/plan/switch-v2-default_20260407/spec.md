# Specification: Switch default prompt to V2

**Track ID:** switch-v2-default_20260407
**Type:** Enhancement
**Created:** 2026-04-07

## Summary

V2 prompt (annotation-driven, no decision tree) outperforms explicit prompt on key tasks (t24, t36).
If full benchmark confirms ≥ 75% on Nemotron, switch Nemotron default to prompt_mode = "v2".
Keep explicit as fallback for providers that need it.

## Acceptance Criteria

- [ ] Full Nemotron v2 benchmark ≥ 75% (30/40)
- [ ] Switch `prompt_mode` default in config.toml nemotron section
- [ ] Verify GPT-5.4 still works with both prompts
- [ ] Update CLAUDE.md with prompt mode documentation
