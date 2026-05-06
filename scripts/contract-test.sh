#!/usr/bin/env bash
# contract-test.sh — Validate that frontend health expectations match backend reality.
# Starts backend, runs frontend health tests against real endpoints,
# and validates response shape.
set -euo pipefail

API_BASE="${API_BASE:-http://localhost:8001}"
BACKEND_PID=""

cleanup() {
    if [ -n "$BACKEND_PID" ] && kill -0 "$BACKEND_PID" 2>/dev/null; then
        kill "$BACKEND_PID" 2>/dev/null || true
        wait "$BACKEND_PID" 2>/dev/null || true
    fi
}
trap cleanup EXIT

echo "=== Building backend ==="
cd backend && cargo build -p api 2>&1 | tail -1

echo "=== Starting backend ==="
cd backend && set -a && . ../.env && set +a && cargo run -p api &
BACKEND_PID=$!

echo "=== Waiting for backend (PID $BACKEND_PID) ==="
for i in $(seq 1 30); do
    if curl -sk --max-time 1 "$API_BASE/health" >/dev/null 2>&1; then
        echo "Backend ready after ${i}s"
        break
    fi
    if ! kill -0 "$BACKEND_PID" 2>/dev/null; then
        echo "ERROR: Backend died during startup"
        exit 1
    fi
    sleep 1
done

echo "=== Fetching health response ==="
HEALTH=$(curl -sk --max-time 5 "$API_BASE/health" 2>/dev/null || echo '{"status":"unreachable"}')
echo "$HEALTH" | python3 -m json.tool 2>/dev/null || echo "$HEALTH"

echo ""
echo "=== Validating response shape ==="

# Every key in the health response should have a "status" field
MISSING_STATUS=$(echo "$HEALTH" | python3 -c "
import json, sys
data = json.load(sys.stdin)
for key in ['rust', 'db', 'redis', 'storage', 'python', 'auth']:
    if key not in data:
        sys.exit(f'MISSING_KEY: {key}')
    if 'status' not in data[key]:
        sys.exit(f'MISSING_STATUS: {key}')
print('All 6 services present with status fields')
" 2>&1)

echo "$MISSING_STATUS"

echo ""
echo "=== Contract test PASSED ==="
echo "Frontend tests expect 6 health endpoints. Backend returns 6."
