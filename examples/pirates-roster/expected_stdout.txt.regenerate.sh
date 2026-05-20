#!/usr/bin/env bash
# Regenerate the golden stdout file for the examples/pirates-roster project.
#
# Run this ONLY when a demo behavior change is intentional (e.g., a new M-section
# was added or output format changed). After running, review the diff and commit
# the updated expected_stdout.txt along with the source change.
#
# Usage: bash examples/pirates-roster/expected_stdout.txt.regenerate.sh
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
source "$HOME/.cargo/env" 2>/dev/null || true
"$REPO_ROOT/target/debug/ynz" run "$SCRIPT_DIR/" 2>/dev/null > "$SCRIPT_DIR/expected_stdout.txt"
echo "Regenerated $SCRIPT_DIR/expected_stdout.txt"
