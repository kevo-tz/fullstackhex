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
    local missing=0;
    
    if ! command -v bombardier &> /dev/null; then
        echo -e "${RED}✗ bombardier not found${NC}"
        echo -e "  Install: go install github.com/codesenberg/bombardier@latest"
        echo -e "  Or run: $0 --install-deps"
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
    local p50=$(echo "$output" | awk '/p50:/ {for(i=1;i<=NF;i++) if($i == "p50:") {gsub(/ms|,/, "", $(i+1)); print $(i+1); exit}}')
    local p99=$(echo "$output" | awk '/p99:/ {for(i=1;i<=NF;i++) if($i == "p99:") {gsub(/ms|,/, "", $(i+1)); print $(i+1); exit}}')
    local rps=$(echo "$output" | awk '/Requests\/sec:/ {for(i=1;i<=NF;i++) if($i == "Requests/sec:") {print $(i+1); exit}}')
    
    # Note: bombardier outputs latency in ms (no conversion needed)
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

# Install dependencies
install_deps() {
    echo -e "${YELLOW}Installing dependencies...${NC}"
    
    if ! command -v go &> /dev/null; then
        echo -e "${RED}✗ Go not found. Install Go first: https://go.dev/dl/${NC}"
        exit 1
    fi
    
    echo -e "Installing bombardier..."
    go install github.com/codesenberg/bombardier@latest
    
    if command -v bombardier &> /dev/null; then
        echo -e "${GREEN}✓ bombardier installed successfully${NC}"
    else
        echo -e "${RED}✗ Failed to install bombardier${NC}"
        exit 1
    fi
}

# Main
main() {
    # Check for --install-deps flag
    if [[ "$1" == "--install-deps" ]]; then
        install_deps
        exit 0
    fi
    
    check_deps
    check_services
    
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