#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/config.sh"

REPO_ROOT="$(get_repo_root)"
cd "$REPO_ROOT"

load_env

for pidfile in "$PID_DIR"/*.pid; do
    if [ -f "$pidfile" ]; then
        read -r pid < "$pidfile" 2>/dev/null || true
        kill "$pid" 2>/dev/null || true
        rm -f "$pidfile"
    fi
done

pkill -x uvicorn 2>/dev/null || true
pkill -x api 2>/dev/null || true
pkill -x bun 2>/dev/null || true

$COMPOSE_DEV down
$COMPOSE_MON down

rm -f "$PYTHON_SOCK"

log_success "All services stopped"
