#!/bin/bash
# Leaderboard submission script
# Usage: ./scripts/leaderboard.sh [provider] [parallel] [name]
# Example: ./scripts/leaderboard.sh nemotron 5 "final-v3"

set -euo pipefail
source .env 2>/dev/null || true

PROVIDER="${1:-nemotron}"
PARALLEL="${2:-5}"
NAME="${3:-$(echo "$PROVIDER-$(date +%m%d-%H%M)")}"
LOG="/tmp/leaderboard-${NAME}.log"

echo "=== Leaderboard: $NAME | Provider: $PROVIDER | Parallel: $PARALLEL ==="
echo "Log: $LOG"

# Kill any other pac1 processes to free CF bandwidth
pkill -f "pac1.*--task" 2>/dev/null || true
pkill -f "pac1.*--parallel" 2>/dev/null || true
sleep 1

# Build release
cargo build --release 2>&1 | tail -1

# Run
LEADERBOARD_PARALLEL=$PARALLEL cargo run --release -- \
  --provider "$PROVIDER" \
  --run "$NAME" \
  2>&1 | tee "$LOG"

echo "=== Done: $NAME ==="
echo "Scores: $(grep -c 'Score: 1' "$LOG")/$(grep -c 'Score:' "$LOG") pass"
echo "Fails: $(grep 'Score: 0' "$LOG" | awk '{print $1}' | tr '\n' ' ')"
