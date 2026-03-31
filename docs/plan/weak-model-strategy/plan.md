# Implementation Plan: Weak Model Strategy

**Track ID:** weak-model-strategy_20260331
**Created:** 2026-03-31
**Status:** [ ] Not Started

## Overview
Improve PAC1 agent from 60% to 80%+ on weak models (Nemotron 120B, Kimi K2.5) via rule-based pre-scan hardening, explicit prompts, and defense-in-depth architecture.

## Phase 1: Hardened Pre-Scan (Rule-Based)
Highest ROI — catches traps deterministically, benefits ALL models.

### Tasks
- [x] Task 1.1: Always scan inbox (remove keyword gate `inbox`/`process`) <!-- sha:2ed01c0 -->
- [x] Task 1.2: Expand injection markers (proximity patterns, HTML attrs, social engineering) <!-- sha:2ed01c0 -->
- [x] Task 1.3: Expand non-CRM markers (OTP, math + digits, trivia patterns) <!-- sha:2ed01c0 -->
- [x] Task 1.4: Add threat scoring (3=DENIED, 2=CLARIFICATION, 1=warn) <!-- sha:2ed01c0 -->

### Verification
- [ ] Pre-scan catches t09 (script injection), t21 (math puzzle)
- [ ] No false positives on t01-t17 (legitimate CRM tasks)

## Phase 2: Prompt Engineering
Make system prompt and tool descriptions explicit enough for weak models.

### Tasks
- [x] Task 2.1: Decision tree in system prompt (numbered steps, not vague bullets) <!-- sha:2ed01c0 -->
- [x] Task 2.2: Enhanced answer tool description with outcome examples <!-- sha:2ed01c0 -->
- [x] Task 2.3: Security hint injection after inbox file pre-load <!-- sha:7695ec5 -->
- [x] Task 2.4: Security check via system prompt decision tree + inbox hint (agent-side, not sgr-agent core) <!-- sha:7695ec5 -->

### Verification
- [x] Nemotron correctly flags injection tasks without pre-scan
- [x] Answer tool outcomes match expected for trap tasks

## Phase 3: Defense in Depth

### Tasks
- [x] Task 3.1: Post-read security guard in agent loop (check read results for injection) <!-- sha:113c99d -->
- [x] Task 3.2: Improve guess_outcome — scan full message history <!-- sha:28c116d -->
- [x] Task 3.3: Model-specific prompt_mode in config.toml (explicit vs standard) <!-- sha:2314eca -->

### Verification
- [x] Agent catches injection discovered mid-loop (not in pre-scan)
- [x] Auto-answer fallback picks correct outcome

## Phase 4: Testing

### Tasks
- [ ] Task 4.1: Unit tests for security scanner (patterns, edge cases, false positives)
- [ ] Task 4.2: --dry-run mode for pre-scan testing without LLM

### Verification
- [ ] cargo test passes
- [ ] --dry-run shows correct pre-scan decisions for all 25 tasks

## Final Verification
- [ ] Nemotron 120B: 70%+ on 25 tasks
- [ ] OpenAI gpt-5.4-mini: maintain 68%+
- [ ] No false positives on legitimate CRM tasks
- [ ] All tests pass

## Context Handoff
### Session Intent
Improve weak model performance on PAC1 trap tasks via rule-based scanning and explicit prompts.

### Key Files
- `src/main.rs` — pre-scan, system prompt, inbox scanning, guess_outcome
- `src/tools.rs` — answer tool description
- `src/config.rs` — prompt_mode config
- sgr-agent `agents/hybrid.rs` — reasoning tool schema, security_check

### Decisions Made
- Rule-based pre-scan over LLM classification — deterministic, zero cost, benefits all models
- Decision tree prompt over vague bullets — weak models follow numbered steps better
- Always scan inbox — remove keyword gate, cost is 1 pcm.list() call

### Risks
- False positives: "override" in legit CRM tasks (e.g. "override phone number")
- Proximity patterns may be too broad — need testing
