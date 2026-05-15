#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/config.sh"

REPO_ROOT="$(get_repo_root)"
cd "$REPO_ROOT"

load_env
# Re-evaluate PYTHON_SOCK now that .env loaded PYTHON_SIDECAR_SOCKET
PYTHON_SOCK="${PYTHON_SOCK:-${PYTHON_SIDECAR_SOCKET:-/tmp/fullstackhex-python.sock}}"

$COMPOSE_DEV down -v --remove-orphans
$COMPOSE_MON down -v --remove-orphans
rm -rf "$PID_DIR"
rm -f "$PYTHON_SOCK"

log_success "Cleaned up all services and volumes"
