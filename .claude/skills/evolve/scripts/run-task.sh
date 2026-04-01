#!/usr/bin/env bash
# Run a PAC1 task and extract score.
# Usage: ./run-task.sh <provider> <task-id>
set -euo pipefail

PROVIDER="${1:?Usage: run-task.sh <provider> <task-id>}"
TASK="${2:?Usage: run-task.sh <provider> <task-id>}"
LOG="/tmp/evolve-${TASK}.log"

cargo build 2>&1 | tail -3
cargo run -- --provider "$PROVIDER" --task "$TASK" 2>&1 | tee "$LOG"
echo "---"
grep "${TASK} Score:" "$LOG" || echo "NO SCORE FOUND"
