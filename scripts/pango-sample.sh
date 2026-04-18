#!/usr/bin/env bash
# Quick sample: 5 diverse tasks in parallel for fast iteration feedback.
# Runs in ~2 min. Exits with count of passes out of 5.
#
# Usage:
#   scripts/pango-sample.sh             # py variant (default)
#   VARIANT=js scripts/pango-sample.sh  # js variant

set -u
VARIANT=${VARIANT:-py}
PROVIDER=${PROVIDER:-cf-gemma4}
TASKS=${TASKS:-t014 t015 t023 t035 t042}  # OCR, trap, security, multilingual, NORA
MAX_ITER=${MAX_ITER:-10}

if [ "$VARIANT" = "py" ]; then
  BIN=pangolin-py-bench
  PREFIX=pysample
else
  BIN=pangolin-bench
  PREFIX=jssample
fi

# Build once.
cargo build --release --bin "$BIN" 2>&1 | grep -E "^error" || true

pids=()
for t in $TASKS; do
  cargo run --release --bin "$BIN" -- --provider "$PROVIDER" --task "$t" --max-iter "$MAX_ITER" \
    > "/tmp/${PREFIX}_${t}.log" 2>&1 &
  pids+=($!)
done
wait "${pids[@]}"

pass=0
for t in $TASKS; do
  log="/tmp/${PREFIX}_${t}.log"
  score=$(grep -oE "Score: [01]\.[0-9]+" "$log" | head -1 | awk '{print $2}')
  outcome=$(grep -oE "OUTCOME_[A-Z_]+" "$log" | tail -1)
  detail=$(grep "^ *•" "$log" | head -1 | sed 's/^ *• //' | cut -c1-80)
  status="✗"
  if [ "$score" = "1.00" ]; then status="✓"; pass=$((pass+1)); fi
  printf "%s %-6s %s %-28s %s\n" "$status" "$t" "$score" "${outcome:-?}" "${detail:-}"
done
echo "---"
echo "$pass / $(echo "$TASKS" | wc -w) passed"
