#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/config.sh"

REPO_ROOT="$(get_repo_root)"
cd "$REPO_ROOT"

load_env
# Re-evaluate PYTHON_SOCK now that .env loaded PYTHON_SIDECAR_SOCKET
PYTHON_SOCK="${PYTHON_SOCK:-${PYTHON_SIDECAR_SOCKET:-/tmp/fullstackhex-python.sock}}"

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
# Catch orphaned Astro/node processes bun leaves behind on abnormal exit
pkill -f "astro dev" 2>/dev/null || true

$COMPOSE_DEV down
$COMPOSE_MON ps -q 2>/dev/null | grep -q . && $COMPOSE_MON down || true

rm -f "$PYTHON_SOCK"

log_success "All services stopped"
