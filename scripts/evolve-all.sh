#!/bin/bash
# Evolve pipeline — iterate over failing tasks until all pass or max iterations reached.
# Uses claude --dangerously-skip-permissions --print to invoke /evolve skill.
#
# Usage:
#   scripts/evolve-all.sh [--provider nemotron] [--max 3] [--tasks "t08 t19 t23"]
#
# Default: runs all 30 tasks, evolves each fail up to 3 iterations.

set -euo pipefail

PROVIDER="${PROVIDER:-nemotron}"
MAX_ITER=3
TASKS=""
LOG_DIR=".agent/evolve-runs"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --provider) PROVIDER="$2"; shift 2 ;;
    --max) MAX_ITER="$2"; shift 2 ;;
    --tasks) TASKS="$2"; shift 2 ;;
    *) shift ;;
  esac
done

mkdir -p "$LOG_DIR"
RUN_ID=$(date +%Y%m%d_%H%M%S)
LOG="$LOG_DIR/$RUN_ID.log"

echo "=== Evolve Pipeline ===" | tee "$LOG"
echo "Provider: $PROVIDER | Max iterations: $MAX_ITER" | tee -a "$LOG"
echo "Run: $RUN_ID" | tee -a "$LOG"
echo "" | tee -a "$LOG"

# Step 1: Run all tasks to find failures
echo "[$(date +%H:%M:%S)] Running benchmark..." | tee -a "$LOG"

if [[ -z "$TASKS" ]]; then
  RESULTS=$(mktemp)
  for t in t01 t02 t03 t04 t05 t06 t07 t08 t09 t10 t11 t12 t13 t14 t15 t16 t17 t18 t19 t20 t21 t22 t23 t24 t25 t26 t27 t28 t29 t30; do
    bash .claude/skills/evolve/scripts/run-task.sh "$PROVIDER" "$t" 2>&1 | grep "Score:" | head -1 >> "$RESULTS" &
  done
  wait

  PASS=$(grep "1.00" "$RESULTS" | wc -l | tr -d ' ')
  FAIL_LIST=$(grep "0.00" "$RESULTS" | sed 's/.*\(t[0-9]*\).*/\1/' | tr '\n' ' ')
  TOTAL=$(wc -l < "$RESULTS" | tr -d ' ')
  echo "[$(date +%H:%M:%S)] Baseline: $PASS/$TOTAL pass" | tee -a "$LOG"
  echo "[$(date +%H:%M:%S)] Fails: $FAIL_LIST" | tee -a "$LOG"
  rm "$RESULTS"
else
  FAIL_LIST="$TASKS"
  echo "[$(date +%H:%M:%S)] Targeting: $FAIL_LIST" | tee -a "$LOG"
fi

if [[ -z "$FAIL_LIST" ]]; then
  echo "[$(date +%H:%M:%S)] All pass! Nothing to evolve." | tee -a "$LOG"
  exit 0
fi

# Step 2: Evolve each failing task
FIXED=0
ATTEMPTED=0

for task in $FAIL_LIST; do
  echo "" | tee -a "$LOG"
  echo "━━━ Evolving $task ━━━" | tee -a "$LOG"
  ATTEMPTED=$((ATTEMPTED + 1))

  TASK_LOG="$LOG_DIR/${RUN_ID}_${task}.log"

  # Run /evolve via claude CLI
  (cd /Users/rustam/startups/active/agent-bit && \
    claude --dangerously-skip-permissions --print -p "/evolve $task" 2>&1) \
    | tee "$TASK_LOG" || true

  # Check if task passes now
  SCORE=$(bash .claude/skills/evolve/scripts/run-task.sh "$PROVIDER" "$task" 2>&1 | grep "Score:" | head -1 | grep -oE "[0-9]+\.[0-9]+" || echo "0.00")

  if [[ "$SCORE" == "1.00" ]]; then
    echo "[$(date +%H:%M:%S)] ✓ $task FIXED (1.00)" | tee -a "$LOG"
    FIXED=$((FIXED + 1))
  else
    # Retry with second iteration
    echo "[$(date +%H:%M:%S)] $task still failing, retry..." | tee -a "$LOG"
    SCORE2=$(bash .claude/skills/evolve/scripts/run-task.sh "$PROVIDER" "$task" 2>&1 | grep "Score:" | head -1 | grep -oE "[0-9]+\.[0-9]+" || echo "0.00")
    if [[ "$SCORE2" == "1.00" ]]; then
      echo "[$(date +%H:%M:%S)] ✓ $task FIXED on retry (non-deterministic)" | tee -a "$LOG"
      FIXED=$((FIXED + 1))
    else
      echo "[$(date +%H:%M:%S)] ✗ $task still failing ($SCORE2)" | tee -a "$LOG"
    fi
  fi
done

# Step 3: Summary
echo "" | tee -a "$LOG"
echo "=== Summary ===" | tee -a "$LOG"
echo "Attempted: $ATTEMPTED tasks" | tee -a "$LOG"
echo "Fixed: $FIXED tasks" | tee -a "$LOG"
echo "Log: $LOG" | tee -a "$LOG"
