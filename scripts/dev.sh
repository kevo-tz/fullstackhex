#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/config.sh"

REPO_ROOT="$(get_repo_root)"
export REPO_ROOT
cd "$REPO_ROOT"

load_env
# Re-evaluate PYTHON_SOCK now that .env loaded PYTHON_SIDECAR_SOCKET
PYTHON_SOCK="${PYTHON_SOCK:-${PYTHON_SIDECAR_SOCKET:-/tmp/fullstackhex-python.sock}}"

WATCH_MODE=false
if [ "${1:-}" = "--watch" ]; then
    WATCH_MODE=true
    if ! command -v cargo-watch &>/dev/null; then
        log_error "cargo-watch not found. Install: cargo install cargo-watch"
        exit 1
    fi
fi

cleanup() {
    log_info "Shutting down..."
    "$SCRIPT_DIR/down.sh"
}
trap cleanup EXIT INT TERM

for tool in bun uv cargo docker; do
    if ! command -v "$tool" &>/dev/null; then
        log_error "$tool not found"
        exit 1
    fi
done
if ! docker compose version &>/dev/null; then
    log_error "docker compose not found"
    exit 1
fi

FAIL=0
if ss -tln 2>/dev/null | grep -q ":8001 " || netstat -tln 2>/dev/null | grep -q ".8001 "; then
    log_error "Port 8001 is in use — run 'make down' first"
    FAIL=1
fi
if ss -tln 2>/dev/null | grep -q ":4321 " || netstat -tln 2>/dev/null | grep -q ".4321 "; then
    log_error "Port 4321 is in use — run 'make down' first"
    FAIL=1
fi
if [ -e "$PYTHON_SOCK" ]; then
    if ss -xl 2>/dev/null | grep -q "$PYTHON_SOCK" || netstat -xl 2>/dev/null | grep -q "$PYTHON_SOCK"; then
        log_error "Socket $PYTHON_SOCK is in use — run 'make down' first"
        FAIL=1
    else
        log_info "Stale socket $PYTHON_SOCK detected — cleaning up"
        rm -f "$PYTHON_SOCK"
    fi
fi
if [ "$FAIL" -eq 1 ]; then
    exit 1
fi
log_success "Preflight passed"

mkdir -p "$PID_DIR"
rm -f "$PID_DIR"/*.pid "$PYTHON_SOCK"

# Generate temporary Redis config to avoid exposing password in ps aux
mkdir -p .tmp
cat > .tmp/redis.conf <<REDIS_CONF
requirepass ${REDIS_PASSWORD}
appendonly yes
appendfsync everysec
save 900 1
save 300 10
save 60 10000
maxmemory ${REDIS_MAX_MEMORY:-512mb}
maxmemory-policy ${REDIS_MAXMEMORY_POLICY:-allkeys-lru}
REDIS_CONF

log_info "Starting infrastructure (PostgreSQL, Redis)..."
$COMPOSE_DEV up -d

log_info "Waiting for PostgreSQL (up to $((POSTGRES_RETRIES * POSTGRES_POLL_INTERVAL))s)..."
for _ in $(seq 1 "$POSTGRES_RETRIES"); do
    if docker compose -f compose/dev.yml exec -T postgres pg_isready -U app_user 2>/dev/null; then
        log_success "PostgreSQL ready"
        break
    fi
    sleep "$POSTGRES_POLL_INTERVAL"
done

log_info "Ensuring PostgreSQL password matches .env (handles stale volumes)..."
$COMPOSE_DEV exec -T postgres psql -U "$POSTGRES_USER" -d "$POSTGRES_DB" \
    -c "ALTER USER \"$POSTGRES_USER\" PASSWORD '$POSTGRES_PASSWORD'" 2>/dev/null || {
    log_error "PostgreSQL password sync failed — auth will fail on API calls"
    exit 1
}

log_info "Starting Python sidecar..."
(cd py-api && uv run uvicorn app.main:app --uds "$PYTHON_SOCK") &
echo $! > "$PID_DIR/python.pid"

log_info "Starting frontend..."
(cd frontend && bun run dev) &
echo $! > "$PID_DIR/frontend.pid"

BACKEND_CMD=(cargo run -p api)
if [ "$WATCH_MODE" = true ]; then
    BACKEND_CMD=(cargo watch -x "run -p api")
fi

log_info "Starting Rust backend..."
cd backend
nohup "${BACKEND_CMD[@]}" > "$PID_DIR/backend.log" 2>&1 &
echo $! > "$PID_DIR/backend.pid"
cd "$REPO_ROOT"

sleep "$POST_START_DELAY"

log_info "Verifying service health (timeout: 30s)..."
TIMEOUT=30
START=$(date +%s)
while true; do
    NOW=$(date +%s)
    ELAPSED=$((NOW - START))
    if [ "$ELAPSED" -ge "$TIMEOUT" ]; then
        log_error "Health check timed out after ${TIMEOUT}s"
        exit 1
    fi
    if curl -sf --max-time 5 http://localhost:8001/health 2>/dev/null | grep -q '"rust".*"status":"ok"'; then
        log_success "All services healthy (${ELAPSED}s)"
        break
    fi
    printf "." >&2
    sleep 1
done

echo ""
echo "=============================================="
echo "  All services healthy. Dashboard:"
echo "  → http://localhost:4321"
echo "=============================================="
echo "Backend logs: $PID_DIR/backend.log"
echo ""
echo "Press Ctrl+C to stop all services."

wait
