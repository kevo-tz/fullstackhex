#!/usr/bin/env bash
# deploy-verify.sh — Poll health endpoints until all services report OK or timeout.
#
# Usage: ./scripts/deploy-verify.sh [--timeout SECONDS] [--base-url URL]
set -euo pipefail

TIMEOUT=60
BASE_URL="${BASE_URL:-http://localhost:8001}"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --timeout) TIMEOUT="$2"; shift 2 ;;
        --base-url) BASE_URL="$2"; shift 2 ;;
        *) echo "Unknown arg: $1"; exit 1 ;;
    esac
done

echo "Verifying service health (timeout: ${TIMEOUT}s)..."

START=$(date +%s)
while true; do
    NOW=$(date +%s)
    ELAPSED=$((NOW - START))
    if [ "$ELAPSED" -ge "$TIMEOUT" ]; then
        echo ""
        echo "Health check timed out after ${TIMEOUT} seconds."
        echo "Failing endpoints:"
        curl -sk --max-time 5 "${BASE_URL}/health" 2>/dev/null || echo "  ${BASE_URL}/health — unreachable"
        echo "Troubleshooting:"
        echo "  - Is the backend running?"
        echo "  - Is PostgreSQL running?"
        exit 1
    fi

    RUST_OK=0
    RUST_STATUS=$(curl -sk --max-time 5 "${BASE_URL}/health" 2>/dev/null | python3 -c "import json,sys; d=json.load(sys.stdin); print(d.get('status',''))" 2>/dev/null || echo "")
    if [ "$RUST_STATUS" = "ok" ]; then RUST_OK=1; fi

    printf "."
    if [ "$RUST_OK" -eq 1 ]; then
        echo ""
        echo "All services healthy (${ELAPSED}s)"
        exit 0
    fi
    sleep 1
done
