#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/config.sh"

REPO_ROOT="$(get_repo_root)"
cd "$REPO_ROOT" || exit

EXIT=0

echo "=== Running Rust tests ==="
(cd backend && cargo test --workspace) || EXIT=$?

echo ""
echo "=== Running Python tests ==="
(cd py-api && uv run pytest) || EXIT=$?

echo ""
echo "=== Running frontend tests (vitest) ==="
(cd frontend && bun run test:vitest) || EXIT=$?

echo ""
if [ "$EXIT" -eq 0 ]; then
    echo "All test suites passed"
else
    echo "Some test suites failed (exit code: $EXIT)"
fi
exit $EXIT
