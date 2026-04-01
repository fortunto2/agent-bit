#!/usr/bin/env bash
# Revert all uncommitted changes (discard failed hypothesis).
# Usage: ./revert.sh
set -euo pipefail

echo "Reverting uncommitted changes..."
git checkout -- .
git status --short
echo "Clean."
