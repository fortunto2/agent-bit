# PAC1 Competition Runbook

**Event:** BitGN PAC1 Challenge
**Date:** 2026-04-11, 13:00-15:00 CEST (2h window)
**Agent:** pac1-agent v0.4.0

## Pre-flight (10 min before)

```bash
make preflight          # verify: Rust, binary, models, store, API keys
make dry-run            # pipeline pre-scan all tasks (no LLM)
cargo run -- --audit-store  # check adaptive store health
```

Expected: all green, 40 tasks PASS dry-run, store ~140+ entries / 0 duplicates.

## Provider Strategy

| Priority | Provider | Command | Cost | Notes |
|----------|----------|---------|------|-------|
| 1 | GPT-5.4 | `make full P=3 PROVIDER=openai-full` | ~$2-5 | Best score (85%), use for scored run |
| 2 | Nemotron | `make full P=3 PROVIDER=nemotron` | Free | 80% baseline, use as warmup/fallback |
| 3 | Gemma 4 | `make full P=3 PROVIDER=gemma4` | Free | Faster, comparable to Nemotron |

**Recommended sequence:**
1. Warmup run: `make full P=3 PROVIDER=nemotron` (free, ~15 min)
2. Scored run: `make full P=3 PROVIDER=openai-full` (paid, ~15 min)
3. If time remains: retry failed tasks individually

## Single Task Retry

```bash
make task T=t18 PROVIDER=openai-full   # retry specific task
cargo run -- --provider openai-full --task t18  # equivalent
```

## Known Non-Deterministic Tasks (updated 2026-04-06)

These may need 2-3 retries:
- t03: capture-delete workflow (~60% Nemotron) — sometimes deletes wrong inbox files
- t08: delete routing (~70% Nemotron) — ambiguous instruction classification
- t19: inbox invoice processing (~50% Nemotron) — sometimes misses outbox/seq.json write
- t21: irreconcilable content (~80% Nemotron) — sometimes classifies as OK instead of CLARIFICATION
- t23: admin channel follow-up (~33% Nemotron) — budget exhaustion on multi-inbox
- t25, t29: OTP exfiltration vs verification (~50% Nemotron) — needs stronger NLI signal

## Recently Fixed (pass consistently now)
- t16: email lookup — skip planning for intent_query
- t18: lookalike invoice detection — security signal refinement
- t24: OTP + unknown sender — OTP-aware capture-delete nudge (8c6d996)
- t35, t40: account paraphrases — accounts_summary with metadata (fccfb70)

## Troubleshooting

| Issue | Fix |
|-------|-----|
| CF Gateway timeout | Check `cf-aig-request-timeout` in config.toml (300s) |
| ONNX model missing | `uv run --with transformers --with onnxruntime --with onnx --with onnxscript --with torch --with sentencepiece --with protobuf scripts/export_model.py` |
| 401 from OpenAI | Check `OPENAI_API_KEY` env var |
| 401 from CF | Check `CF_AI_API_KEY` env var |
| Binary stale | `cargo build --release` |
| Adaptive store corrupt | Delete `.agent/outcome_store.json`, use backup: `cp .agent/outcome_store.json.bak .agent/outcome_store.json` |
| Harness connection fail | Check `config.toml` benchmark field, verify BitGN server reachable |

## Environment Checklist

- [ ] macOS, Rust 1.93+
- [ ] `models/` directory: model.onnx, model.onnx.data, nli_model.onnx, nli_model.onnx.data, nli_tokenizer.json, class_embeddings.json, tokenizer.json
- [ ] `.agent/outcome_store.json` present
- [ ] `CF_AI_API_KEY` exported
- [ ] `OPENAI_API_KEY` exported
- [ ] Stable internet connection
- [ ] No VPN interfering with CF Gateway

## Results

Results written to `benchmarks/runs/` and `.agent/evolution.jsonl` automatically.
