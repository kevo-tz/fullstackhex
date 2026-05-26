#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/config.sh"

REPO_ROOT="$(get_repo_root)"
cd "$REPO_ROOT"

load_env
# Re-evaluate PYTHON_SOCK now that .env loaded PYTHON_SIDECAR_SOCKET
PYTHON_SOCK="${PYTHON_SOCK:-${PYTHON_SIDECAR_SOCKET:-/tmp/fullstackhex-python.sock}}"

log_info "Stopping services..."

# Kill by PID files (clean shutdown)
for pidfile in "$PID_DIR"/*.pid; do
    if [ -f "$pidfile" ]; then
        read -r pid < "$pidfile" 2>/dev/null || true
        if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then
            kill "$pid" 2>/dev/null || true
        fi
        rm -f "$pidfile"
    fi
done

# Kill by process name (catch orphans / non-PID-managed processes)
pkill -f "uvicorn" 2>/dev/null || true
pkill -f "target/debug/api" 2>/dev/null || true
pkill -f "bun run dev" 2>/dev/null || true
pkill -f "astro dev" 2>/dev/null || true

# Force-free ports used by dev stack
fuser -k 8001/tcp 2>/dev/null || true
fuser -k 4321/tcp 2>/dev/null || true

# Docker compose services
$COMPOSE_DEV down 2>/dev/null || true
$COMPOSE_MON ps -q 2>/dev/null | grep -q . && $COMPOSE_MON down 2>/dev/null || true

# Cleanup socket and PID dir
rm -f "$PYTHON_SOCK"
rm -rf "$PID_DIR"

# Brief wait for ports to drain
sleep 1

# Verify ports free
if fuser 8001/tcp 4321/tcp 2>/dev/null | grep -q .; then
    log_warning "Some ports still in use — retrying with SIGKILL"
    fuser -k -9 8001/tcp 4321/tcp 2>/dev/null || true
    sleep 1
fi

log_success "All services stopped, ports freed"
