#!/bin/bash
set -e

# FullStackHex Performance Benchmark Script (Lite version)
# Usage: ./scripts/bench-lite.sh
# Requires: ab (Apache Bench) - install via: apt-get install apache2-utils (Linux) or yum install httpd-tools (RHEL)

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

RUST_BACKEND_URL="${RUST_BACKEND_URL:-http://localhost:8001}"
FRONTEND_URL="${FRONTEND_URL:-http://localhost:4321}"
REQUESTS="${REQUESTS:-1000}"
CONCURRENT="${CONCURRENT:-100}"

echo -e "${YELLOW}========================================${NC}"
echo -e "${YELLOW}  FullStackHex Performance Benchmarks${NC}"
echo -e "${YELLOW}  (Lite - using Apache Bench)${NC}"
echo -e "${YELLOW}========================================${NC}"
echo ""

# Check dependencies
check_deps() {
    local missing=0;

    if ! command -v ab &> /dev/null; then
        echo -e "${RED}✗ ab (Apache Bench) not found${NC}"
        echo -e "  Install:"
        echo -e "    Linux (Debian/Ubuntu): sudo apt-get install apache2-utils"
        echo -e "    Linux (RHEL/CentOS): sudo yum install httpd-tools"
        echo -e "    macOS: ab is included with Apache (or brew install httpd)"
        missing=1
    else
        local version=$(ab -V 2>&1 | head -1)
        echo -e "${GREEN}✓ ab found: $version${NC}"
    fi

    if ! command -v curl &> /dev/null; then
        echo -e "${RED}✗ curl not found${NC}"
        missing=1
    else
        echo -e "${GREEN}✓ curl found${NC}"
    fi

    if [ $missing -eq 1 ]; then
        exit 1
    fi
}

# Check if services are running
check_services() {
    local failed=0

    echo ""
    echo -e "${YELLOW}Checking services...${NC}"

    # Check Rust backend
    if curl --silent --fail "$RUST_BACKEND_URL/health" > /dev/null 2>&1; then
        echo -e "${GREEN}✓ Rust backend responding at $RUST_BACKEND_URL${NC}"
    else
        echo -e "${RED}✗ Rust backend not responding at $RUST_BACKEND_URL${NC}"
        echo -e "${YELLOW}  Start with: cd backend && cargo run -p api${NC}"
        failed=1
    fi

    # Check Frontend
    if curl --silent --fail "$FRONTEND_URL" > /dev/null 2>&1; then
        echo -e "${GREEN}✓ Frontend responding at $FRONTEND_URL${NC}"
    else
        echo -e "${RED}✗ Frontend not responding at $FRONTEND_URL${NC}"
        echo -e "${YELLOW}  Start with: cd frontend && bun run dev${NC}"
        failed=1
    fi

    if [ $failed -eq 1 ]; then
        echo ""
        echo -e "${RED}Please start all services before running benchmarks.${NC}"
        exit 1
    fi

    echo ""
}

# Benchmark function using ab
benchmark() {
    local name="$1"
    local url="$2"
    local expected_p50="$3"
    local expected_p99="$4"

    echo ""
    echo -e "${YELLOW}Benchmark: $name${NC}"
    echo -e "URL: $url"
    echo -e "Requests: $REQUESTS, Concurrent: $CONCURRENT"
    echo ""

    # Run ab and capture output
    local output=$(ab -n "$REQUESTS" -c "$CONCURRENT" -r -k "$url" 2>&1)

    # Parse p50 and p99 from "Percentage of requests served within a certain time" table
    # Format: "  50%    123" (ms)
    local p50=$(echo "$output" | awk '/^ +50% / {print $2}' | head -1)
    local p99=$(echo "$output" | awk '/^ +99% / {print $2}' | head -1)

    # If p50/p99 not found, try alternative format using portable awk-only parsing
    if [ -z "$p50" ]; then
        p50=$(echo "$output" | awk '/^ +50% / {for (i = 1; i <= NF; i++) if ($i ~ /^[0-9]+$/) {print $i; exit}}' | head -1)
    fi
    if [ -z "$p99" ]; then
        p99=$(echo "$output" | awk '/^ +99% / {for (i = 1; i <= NF; i++) if ($i ~ /^[0-9]+$/) {print $i; exit}}' | head -1)
    fi

    # Fallback: use mean time if percentiles not available
    if [ -z "$p50" ] || [ -z "$p99" ]; then
        echo -e "${YELLOW}⚠ Could not parse p50/p99 from ab output, using mean time${NC}"
        local mean=$(echo "$output" | awk '/^Time per request:/ {print $4; exit}' | head -1)
        p50=${p50:-$mean}
        p99=${p99:-$mean}
    fi

    echo -e "Results:"
    echo -e "  p50: ${p50}ms (target: <${expected_p50}ms)"
    echo -e "  p99: ${p99}ms (target: <${expected_p99}ms)"

    # Check against targets (convert to integers for comparison)
    local p50_int=$(echo "$p50" | cut -d. -f1)
    local p99_int=$(echo "$p99" | cut -d. -f1)
    local expected_p50_int=$(echo "$expected_p50" | cut -d. -f1)
    local expected_p99_int=$(echo "$expected_p99" | cut -d. -f1)

    local passed=0

    if [ -n "$p50_int" ] && [ "$p50_int" -lt "$expected_p50_int" ] 2>/dev/null; then
        echo -e "  ${GREEN}✓ p50 PASSED${NC}"
    else
        echo -e "  ${RED}✗ p50 FAILED${NC}"
        passed=1
    fi

    if [ -n "$p99_int" ] && [ "$p99_int" -lt "$expected_p99_int" ] 2>/dev/null; then
        echo -e "  ${GREEN}✓ p99 PASSED${NC}"
    else
        echo -e "  ${RED}✗ p99 FAILED${NC}"
        passed=1
    fi

    return $passed
}

# Frontend TTFB benchmark using curl
benchmark_frontend_ttfb() {
    echo ""
    echo -e "${YELLOW}Benchmark: Frontend TTFB (SSR)${NC}"
    echo -e "URL: $FRONTEND_URL"
    echo ""

    local ttfb=$(curl -w "%{time_starttransfer}" -o /dev/null -s "$FRONTEND_URL")
    local expected=0.1  # 100ms

    # Convert to milliseconds for display without requiring bc
    local ttfb_ms
    ttfb_ms=$(awk -v ttfb="$ttfb" 'BEGIN { printf "%.3f", ttfb * 1000 }')
    echo -e "TTFB: ${ttfb}s (${ttfb_ms}ms) (target: <${expected}s)"

    if awk -v ttfb="$ttfb" -v expected="$expected" 'BEGIN { exit !(ttfb < expected) }'; then
        echo -e "${GREEN}✓ TTFB PASSED${NC}"
        return 0
    else
        echo -e "${RED}✗ TTFB FAILED${NC}"
        return 1
    fi
}

# Main
main() {
    check_deps
    check_services

    echo -e "${YELLOW}Configuration:${NC}"
    echo -e "  RUST_BACKEND_URL: $RUST_BACKEND_URL"
    echo -e "  FRONTEND_URL: $FRONTEND_URL"
    echo -e "  Requests: $REQUESTS"
    echo -e "  Concurrent: $CONCURRENT"

    local failed=0

    # 1. /api/health p50/p99 latency
    if ! benchmark "Rust /health endpoint" "$RUST_BACKEND_URL/health" 5 20; then
        failed=1
    fi

    # 2. Frontend TTFB
    if ! benchmark_frontend_ttfb; then
        failed=1
    fi

    echo ""
    echo -e "${YELLOW}========================================${NC}"

    if [ $failed -eq 0 ]; then
        echo -e "${GREEN}  ✓ All benchmarks passed${NC}"
    else
        echo -e "${RED}  ✗ Some benchmarks failed${NC}"
    fi

    echo -e "${YELLOW}========================================${NC}"

    exit $failed
}

main "$@"
