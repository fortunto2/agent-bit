# Implementation Plan: Fix t23 Over-Cautious DENIED on Contact Ambiguity

**Track ID:** fix-t23-contact-ambiguity_20260403
**Spec:** [spec.md](./spec.md)
**Created:** 2026-04-03
**Status:** [~] In Progress (Phase 1-2 complete, Phase 3 blocked by disk space)

## Overview

Pre-resolve contact ambiguity during pre-grounding using the CRM graph, then annotate search results with disambiguation hints. Two layers: (1) inject contact hints before LLM loop, (2) enrich search results when multiple contacts match.

## Phase 1: Contact Pre-Grounding

Extract names from inbox emails and resolve them against the CRM graph before the LLM execution loop. Inject disambiguation hints into pre-grounding context.

### Tasks

- [x] Task 1.1: Add `extract_mentioned_names()` to `src/main.rs` — parse inbox content for person names (From: display name, To: names, and body mentions of known CRM contacts via `CrmGraph.name_index`). Return `Vec<(String, String)>` of (name, source_file).

- [x] Task 1.2: Add `resolve_contact_hints()` to `src/main.rs` — for each extracted name, use `CrmGraph.fuzzy_find_contact()` + `name_index` lookup to find matching contacts. When multiple match, rank by: (1) exact name match, (2) same account as sender domain, (3) fuzzy score. Return formatted hints string.

- [x] Task 1.3: Add `pub fn contacts_for_account(&self, account_name: &str) -> Vec<String>` to `src/crm_graph.rs` — traverse graph edges to find all contacts linked to an account via `WorksAt`. Needed for disambiguation by account affiliation.

- [x] Task 1.4: Inject contact hints into pre-grounding messages in `run_agent()` (`src/main.rs` ~line 1316). After inbox content injection, call `resolve_contact_hints()` and push as a user message: "Contact hints: [name] → best match: [contact] (account: [X])".

- [x] Task 1.5: Unit tests for `extract_mentioned_names()` — known name in CRM, unknown name, multiple names, no names.

- [x] Task 1.6: Unit tests for `resolve_contact_hints()` — single match (no hint needed), multiple matches (ranked), no match.

- [x] Task 1.7: Unit tests for `contacts_for_account()` in `src/crm_graph.rs` — account with contacts, account with no contacts, nonexistent account. <!-- sha:1da298a -->

### Verification

- [x] `cargo test` passes (all 105 tests)
- [x] `cargo build` succeeds
- [ ] `make task T=t23` passes on Nemotron

## Phase 2: Search Result Disambiguation

When search returns multiple contacts, annotate results with CRM graph context.

### Tasks

- [x] Task 2.1: Add `annotate_contact_results()` helper in `src/tools.rs` — when search root is "contacts" and multiple files match, use CRM graph to add "[BEST MATCH]" annotation based on the current inbox context (sender domain, referenced account).

- [x] Task 2.2: Thread `CrmGraph` (or a lightweight contact ranking closure) into `SearchTool` — add `Option<Arc<CrmGraph>>` field so search can access graph context. Wire it in `run_agent()` at tool registry construction (~line 1369).

- [x] Task 2.3: In `SearchTool::execute()`, after `auto_expand_search()`, call `annotate_contact_results()` when search root starts with "contacts". Append "[BEST MATCH: ...]" to the best-ranked result.

- [x] Task 2.4: Unit test for `annotate_contact_results()` — two contacts same surname different accounts, one matching sender domain. <!-- sha:fab9fb0 -->

### Verification

- [x] `cargo test` passes (105 tests)
- [ ] `make task T=t23` passes on Nemotron
- [ ] `make task T=t23 PROVIDER=openai` passes on GPT-5.4

## Phase 3: Regression Testing & Docs

### Tasks

- [ ] Task 3.1: Run full regression: `make task T=t18` (social engineering), `make task T=t19` (legit resend), `make task T=t01`, `make task T=t09`, `make task T=t16`, `make task T=t24`. All must pass. (BLOCKED: disk full, need `cargo clean` + rebuild)

- [x] Task 3.2: Update `CLAUDE.md` — add contact pre-grounding to Decision Pipeline section, document the new CRM graph method. <!-- sha:ba10bbd -->

- [ ] Task 3.3: Update `docs/roadmap.md` — mark t23 as fixed, update scores if improved.

### Verification

- [ ] All acceptance criteria from spec met
- [ ] Tests pass
- [ ] Linter clean (`cargo clippy`)
- [ ] Build succeeds
- [ ] Documentation up to date

## Final Verification

- [ ] All acceptance criteria from spec met
- [ ] Tests pass
- [ ] Linter clean
- [ ] Build succeeds
- [ ] Documentation up to date

## Context Handoff

_Summary for /build to load at session start — keeps context compact._

### Session Intent

Fix t23 non-deterministic failure caused by contact ambiguity discovered too late in LLM execution loop. Pre-resolve ambiguity during pre-grounding using CRM graph.

### Key Files

- `src/main.rs` — pre-grounding (lines 1245-1380), inbox reader (1003-1081), system prompt (69-105)
- `src/crm_graph.rs` — CRM graph with contact/account/domain nodes
- `src/tools.rs` — SearchTool (lines 400-477), auto_expand_search (437-453)

### Decisions Made

- **Pre-grounding over prompt-only**: Prompt hint (line 79) gives 75% GPT-5.4 but 0% Nemotron. Pre-resolving removes ambiguity before LLM sees it — more deterministic.
- **Two-layer approach**: Pre-grounding handles the common case; search annotation handles edge cases where LLM searches for contacts not mentioned in inbox.
- **CRM graph ranking**: Rank by exact name > account affiliation > fuzzy score. Account affiliation uses sender domain correlation (reuses `check_sender_domain_match` logic).
- **Non-breaking to SearchTool**: CrmGraph threaded as `Option<Arc<CrmGraph>>` — search works without it, just no annotations.

### Risks

- Name extraction from email body is fuzzy — may miss contacts not in CRM or extract false names. Mitigated by only matching against known `name_index` entries.
- Threading CrmGraph into SearchTool adds a dependency — but it's `Option`-wrapped and read-only, so no concurrency issues.
- Over-hinting could bias the LLM to always pick the hinted contact, even when wrong. Mitigated by only adding hints when multiple matches exist (ambiguous case).

---
_Generated by /plan. Tasks marked [~] in progress and [x] complete by /build._
