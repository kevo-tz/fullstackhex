#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/config.sh"

REPO_ROOT="$(get_repo_root)"
cd "$REPO_ROOT" || exit

cleanup() {
    log_info "Stopping log tail..."
    kill 0 2>/dev/null || true
    exit 0
}
trap cleanup INT TERM

if [ -f "$PID_DIR/backend.log" ]; then
    tail -f "$PID_DIR/backend.log" &
else
    log_warning "Backend log not found (start dev first)"
fi

docker compose -f compose/dev.yml logs -f postgres redis &

echo ""
echo "Frontend and Python logs appear in their terminal windows."
echo "Press Ctrl+C to stop log tail."
echo ""

wait
