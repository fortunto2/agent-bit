# Benchmark Results

Per-run logs in `runs/`. Format: `YYYY-MM-DD__{provider}__{commit}.md`

## Latest Baselines

| Provider | Model | Score | Date | Commit |
|----------|-------|-------|------|--------|
| openai | gpt-5.4 | 64.0% (16/25) | 2026-03-31 | 0335320 |
| nemotron | nemotron-120b | 62.5% (5/8) | 2026-03-31 | 0335320 |

## How to run

```bash
# Quick sample (8 tasks)
cargo run -- --provider nemotron --task t01 && cargo run -- --provider nemotron --task t09
# Full benchmark
cargo run -- --provider openai --parallel 3
# Dry-run (pre-scan only, no LLM)
cargo run -- --dry-run --provider nemotron
```

After a run, log it: copy scores into `runs/` using the template below.
