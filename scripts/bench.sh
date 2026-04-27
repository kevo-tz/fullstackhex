#!/bin/bash
set -e

# FullStackHex Performance Benchmark Script
# Usage: ./scripts/bench.sh
# Requires: bombardier (install via: go install github.com/codesenberg/bombardier@latest)

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

RUST_BACKEND_URL="${RUST_BACKEND_URL:-http://localhost:8001}"
FRONTEND_URL="${FRONTEND_URL:-http://localhost:4321}"
DURATION="${DURATION:-30s}"
CONCURRENT="${CONCURRENT:-100}"

echo -e "${YELLOW}========================================${NC}"
echo -e "${YELLOW}  FullStackHex Performance Benchmarks${NC}"
echo -e "${YELLOW}========================================${NC}"
echo ""

# Check dependencies
check_deps() {
    local missing=0
    
    if ! command -v bombardier &> /dev/null; then
        echo -e "${RED}✗ bombardier not found${NC}"
        echo -e "  Install: go install github.com/codesenberg/bombardier@latest"
        missing=1
    else
        echo -e "${GREEN}✓ bombardier found${NC}"
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

# Benchmark function
benchmark() {
    local name="$1"
    local url="$2"
    local expected_p50="$3"
    local expected_p99="$4"
    
    echo ""
    echo -e "${YELLOW}Benchmark: $name${NC}"
    echo -e "URL: $url"
    echo -e "Duration: $DURATION, Concurrent: $CONCURRENT"
    echo ""
    
    local output=$(bombardier -c "$CONCURRENT" -d "$DURATION" "$url" 2>&1)
    
    # Parse results (bombardier outputs to stderr)
    local p50=$(echo "$output" | grep -oP 'Mean:\s*\K[0-9.]+' | head -1)
    local p99=$(echo "$output" | grep -oP '99%\s*\K[0-9.]+' | head -1)
    local rps=$(echo "$output" | grep -oP 'Requests/sec:\s*\K[0-9.]+' | head -1)
    
    # Convert ms toms (bombardier outputs in ms)
    p50="${p50:-0}"
    p99="${p99:-0}"
    
    echo -e "Results:"
    echo -e "  p50: ${p50}ms (target: <${expected_p50}ms)"
    echo -e "  p99: ${p99}ms (target: <${expected_p99}ms)"
    echo -e "  RPS: ${rps}"
    
    # Check against targets
    local passed=0
    local p50_passed=$(echo "$p50 < $expected_p50" | bc -l 2>/dev/null || echo "0")
    local p99_passed=$(echo "$p99 < $expected_p99" | bc -l 2>/dev/null || echo "0")
    
    if [ "$p50_passed" = "1" ]; then
        echo -e "  ${GREEN}✓ p50 PASSED${NC}"
    else
        echo -e "  ${RED}✗ p50 FAILED${NC}"
        passed=1
    fi
    
    if [ "$p99_passed" = "1" ]; then
        echo -e "  ${GREEN}✓ p99 PASSED${NC}"
    else
        echo -e "  ${RED}✗ p99 FAILED${NC}"
        passed=1
    fi
    
    return $passed
}

# Frontend TTFB benchmark
benchmark_frontend_ttfb() {
    echo ""
    echo -e "${YELLOW}Benchmark: Frontend TTFB (SSR)${NC}"
    echo -e "URL: $FRONTEND_URL"
    echo ""
    
    local ttfb=$(curl -w "%{time_starttransfer}" -o /dev/null -s "$FRONTEND_URL")
    local expected=0.1
    
    echo -e "TTFB: ${ttfb}s (target: <${expected}s)"
    
    local passed=$(echo "$ttfb < $expected" | bc -l 2>/dev/null || echo "0")
    
    if [ "$passed" = "1" ]; then
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
    
    echo -e "${YELLOW}Configuration:${NC}"
    echo -e "  RUST_BACKEND_URL: $RUST_BACKEND_URL"
    echo -e "  FRONTEND_URL: $FRONTEND_URL"
    echo -e "  Duration: $DURATION"
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