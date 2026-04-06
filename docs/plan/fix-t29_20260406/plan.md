# Implementation Plan: Fix t29

**Track ID:** fix-t29_20260406
**Spec:** [spec.md](./spec.md)
**Created:** 2026-04-06
**Status:** [~] In Progress

## Phase 1: Diagnose — DONE

- [x] Task 1.1: Root cause — OTP matches but agent answer not precise ("correct"/"incorrect")
- [x] Task 1.2: score_detail: "answer contains expected answer but is not precise. Expected: 'correct'"
- [x] Task 1.3: Agent writes correct answer but wraps it in explanation text instead of bare "correct"

## Phase 2: Fix

- [ ] Task 2.1: Prompt/example: when inbox says "reply with exactly X" → answer message must be EXACTLY that word
- [ ] Task 2.2: Verify t29 passes (2 runs) + t24, t25 regression

## Context Handoff

### Key Files
- `src/prompts.rs` — credential examples, OTP verification example
- Trial dump: agent gets right answer but wraps in explanation
