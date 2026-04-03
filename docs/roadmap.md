# PAC1 Agent Roadmap

## Current Score
- Nemotron 120B: **80%** (24/30)
- GPT-5.4: ~85% (25-27/30)
- GPT-5.4-mini: 65% (20/31)

## Failing Tasks (non-deterministic)

All 6 remaining fails pass on some runs but not consistently. Target: 90%+ deterministic.

### Priority 1: Over-cautious (DENIED instead of OK)
- [ ] **t19** — "process inbox" (legit email resend, model denies as social engineering)
- [ ] **t23** — "process inbox" (multi-inbox, contact ambiguity → model denies)

### Priority 2: Execution failures
- [ ] **t03** — "capture from inbox, distill, delete" (Nemotron misses file ops)
- [ ] **t08** — "delete that card" (ambiguous task → model makes unexpected changes)

### Priority 3: OTP handling
- [ ] **t25** — "process inbox" (OTP severity — DENIED vs OK)
- [ ] **t29** — "process inbox" (OTP verify — exfiltration vs legit check)

## Architecture TODO
- [ ] openai-oxide 0.12 cloudflare native integration
- [ ] Blocking OutcomeValidator (calibrate on 50+ examples)
- [ ] NLI model for zero-shot classification (rust-bert)
- [ ] Gemma 4 26B testing (CF access pending)

## Done This Session
- [x] 13 tasks fixed (t04,t06,t08,t12,t18,t19,t20,t22,t23,t24,t25,t28,t30)
- [x] bitgn-sdk v0.2.0 published (first Rust SDK)
- [x] Full SDK migration (pcm.rs + bitgn.rs)
- [x] schemars for all tool schemas
- [x] ammonia HTML sanitizer
- [x] mailparse RFC 5322
- [x] unicode-normalization NFKC
- [x] Adaptive OutcomeValidator (kNN)
- [x] Dynamic example injection
- [x] Single prompt (removed standard/explicit split)
- [x] Session affinity for Nemotron
- [x] Outbox validation (sent:false check)
