# Specification: Fix t19 Over-Cautious DENIED on Legit Inbox

**Track ID:** fix-t19-overcautious_20260403
**Type:** Bug
**Created:** 2026-04-03
**Status:** Draft

## Summary

t19 "process inbox" is a legitimate email resend task that intermittently fails because the agent answers OUTCOME_DENIED_SECURITY instead of OUTCOME_OK. Root cause: the ensemble blocker (scan_inbox) fires when ML confidence >0.4 + sender is UNKNOWN + content mentions "invoice". This hard-blocks before the LLM sees the self-consistency annotation [SENDER DOMAIN MATCHES].

The body-match fallback (domain stem vs email body) was added but uses strict >0.5 threshold. Non-deterministic ML scores between 0.4-0.6 cause flapping — sometimes blocked, sometimes not.

## Acceptance Criteria

- [x] t19 passes 3/3 runs on Nemotron
- [x] t18 (social engineering) still passes 2/3 (no regression on security)
- [x] t20 (cross-company) still passes (no regression)
- [x] Ensemble blocker only fires on MISMATCH, not UNKNOWN with self-consistent domain
- [x] No new false positives on t01, t09, t16, t24

## Dependencies

- Domain matching: `check_sender_domain_match()` in main.rs
- Ensemble blocker: `scan_inbox()` in main.rs line 554
- Body-match fallback: main.rs line 962

## Out of Scope

- Fixing t23 (different root cause — contact ambiguity)
- NLI model integration
- OutcomeValidator blocking mode

## Technical Notes

- Ensemble blocker runs BEFORE LLM sees annotations — self-consistency not visible
- Body-match fallback uses >0.5 (strict), so "acme" in 2-word stems = 50% = NOT match
- ML classifier non-deterministic: same content scores 0.35-0.55 between runs
- Three fix options identified: raise threshold, exclude unknown, strengthen fallback
