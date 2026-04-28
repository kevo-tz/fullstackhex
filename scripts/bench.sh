#!/bin/bash
set -e

# FullStackHex Performance Benchmark Script
# Usage: ./scripts/bench.sh
# Requires: bombardier (install via: go install github.com/codesenberg/bombardier@latest)

# Source common functions and configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"
source "$SCRIPT_DIR/config.sh"

# Configuration is now sourced from config.sh

log_info "FullStackHex Performance Benchmarks"
echo ""

# Check dependencies
check_deps() {
    local missing=0;
    
    if ! command -v bombardier &> /dev/null; then
        log_error "bombardier not found"
        log_info "Install: go install github.com/codesenberg/bombardier@latest"
        log_info "Or run: $0 --install-deps"
        missing=1
    else
        log_success "bombardier found"
    fi
    
    if ! command -v curl &> /dev/null; then
        log_error "curl not found"
        missing=1
    else
        log_success "curl found"
    fi
    
    if [ $missing -eq 1 ]; then
        exit 1
    fi
}

# Check if services are running
check_services() {
    local failed=0

    log_info "Checking services..."

    # Check Rust backend
    if check_service_http "Rust backend" "$RUST_BACKEND_URL/health" 5 false; then
        log_success "Rust backend responding at $RUST_BACKEND_URL"
    else
        log_error "Rust backend not responding at $RUST_BACKEND_URL"
        log_warning "Start with: cd backend && cargo run -p api"
        failed=1
    fi

    # Check Frontend
    if check_service_http "Frontend" "$FRONTEND_URL" 5 false; then
        log_success "Frontend responding at $FRONTEND_URL"
    else
        log_error "Frontend not responding at $FRONTEND_URL"
        log_warning "Start with: cd frontend && bun run dev"
        failed=1
    fi

    if [ $failed -eq 1 ]; then
        log_error "Please start all services before running benchmarks."
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
    
    log_info "Benchmark: $name"
    log_info "URL: $url"
    log_info "Duration: $DURATION, Concurrent: $CONCURRENT"
    echo ""
    
    local output=$(bombardier -c "$CONCURRENT" -d "$DURATION" "$url" 2>&1)
    
    # Parse results (bombardier outputs to stderr)
    local p50=$(echo "$output" | awk '/p50:/ {for(i=1;i<=NF;i++) if($i == "p50:") {gsub(/ms|,/, "", $(i+1)); print $(i+1); exit}}')
    local p99=$(echo "$output" | awk '/p99:/ {for(i=1;i<=NF;i++) if($i == "p99:") {gsub(/ms|,/, "", $(i+1)); print $(i+1); exit}}')
    local rps=$(echo "$output" | awk '/Requests\/sec:/ {for(i=1;i<=NF;i++) if($i == "Requests/sec:") {print $(i+1); exit}}')
    
    # Note: bombardier outputs latency in ms (no conversion needed)
    p50="${p50:-0}"
    p99="${p99:-0}"
    
    log_info "Results:"
    log_info "  p50: ${p50}ms (target: <${expected_p50}ms)"
    log_info "  p99: ${p99}ms (target: <${expected_p99}ms)"
    log_info "  RPS: ${rps}"
    
    # Check against targets
    local passed=0
    local p50_passed=$(echo "$p50 < $expected_p50" | bc -l 2>/dev/null || echo "0")
    local p99_passed=$(echo "$p99 < $expected_p99" | bc -l 2>/dev/null || echo "0")
    
    if [ "$p50_passed" = "1" ]; then
        log_success "p50 PASSED"
    else
        log_error "p50 FAILED"
        passed=1
    fi
    
    if [ "$p99_passed" = "1" ]; then
        log_success "p99 PASSED"
    else
        log_error "p99 FAILED"
        passed=1
    fi
    
    # Return structured result for JSON output
    echo "{\"name\":\"$name\",\"url\":\"$url\",\"p50_ms\":$p50,\"p99_ms\":$p99,\"rps\":\"$rps\",\"p50_target_ms\":$expected_p50,\"p99_target_ms\":$expected_p99,\"p50_passed\":$p50_passed,\"p99_passed\":$p99_passed,\"passed\":$[ $passed -eq 0 ]}"
    
    return $passed
}

# Frontend TTFB benchmark
benchmark_frontend_ttfb() {
    log_info "Benchmark: Frontend TTFB (SSR)"
    log_info "URL: $FRONTEND_URL"
    echo ""
    
    local ttfb=$(curl -w "%{time_starttransfer}" -o /dev/null -s "$FRONTEND_URL")
    local expected_s=$(echo "scale=3; $FRONTEND_TTFB_THRESHOLD / 1000" | bc)
    
    log_info "TTFB: ${ttfb}s (target: <${expected_s}s)"
    
    local passed=$(echo "$ttfb < $expected_s" | bc -l 2>/dev/null || echo "0")
    
    if [ "$passed" = "1" ]; then
        log_success "TTFB PASSED"
        return 0
    else
        log_error "TTFB FAILED"
        return 1
    fi
}

# Install dependencies
install_deps() {
    log_info "Installing dependencies..."
    
    if ! command -v go &> /dev/null; then
        log_error "Go not found. Install Go first: https://go.dev/dl/${NC}"
        exit 1
    fi
    
    log_info "Installing bombardier..."
    go install github.com/codesenberg/bombardier@latest
    
    if command -v bombardier &> /dev/null; then
        log_success "bombardier installed successfully"
    else
        log_error "Failed to install bombardier"
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
    
    # Check for --help flag
    if [[ "$1" == "--help" || "$1" == "-h" ]]; then
        echo "Usage: $0 [--install-deps] [--json]"
        echo ""
        echo "Options:"
        echo "  --install-deps   Install required dependencies (bombardier)"
        echo "  --json           Output results in JSON format"
        echo "  --help, -h       Show this help message"
        exit 0
    fi
    
    # Check for JSON output flag
    local JSON_OUTPUT=false
    if [[ "$1" == "--json" ]]; then
        JSON_OUTPUT=true
        shift
    fi
    
    check_deps
    check_services
    
    if [ "$JSON_OUTPUT" = false ]; then
        log_info "Configuration:"
        log_info "  RUST_BACKEND_URL: $RUST_BACKEND_URL"
        log_info "  FRONTEND_URL: $FRONTEND_URL"
        log_info "  Duration: $DURATION"
        log_info "  Concurrent: $CONCURRENT"
    fi
    
    # Initialize results for JSON output
    local timestamp=$(get_timestamp)
    local git_commit=$(get_git_commit)
    
    local failed=0
    
    # 1. /api/health p50/p99 latency
    local health_result
    if ! health_result=$(benchmark "Rust /health endpoint" "$RUST_BACKEND_URL/health" "$RUST_HEALTH_P50_THRESHOLD" "$RUST_HEALTH_P99_THRESHOLD"); then
        failed=1
    fi
    
    # 2. Frontend TTFB
    local frontend_result
    if ! frontend_result=$(benchmark_frontend_ttfb); then
        failed=1
    fi
    
    if [ "$JSON_OUTPUT" = true ]; then
        # Output JSON results (simple format without external dependencies)
        echo "{"
        echo "  \"timestamp\": \"$timestamp\","
        echo "  \"git_commit\": \"$git_commit\","
        echo "  \"benchmarks\": ["
        echo "    {"
        echo "      \"name\": \"Rust /health endpoint\","
        echo "      \"url\": \"$RUST_BACKEND_URL/health\","
        echo "      \"duration\": \"$DURATION\","
        echo "      \"concurrent\": \"$CONCURRENT\","
        echo "      \"result\": \"$health_result\""
        echo "    },"
        echo "    {"
        echo "      \"name\": \"Frontend TTFB\","
        echo "      \"url\": \"$FRONTEND_URL\","
        echo "      \"result\": \"$frontend_result\""
        echo "    }"
        echo "  ]"
        echo "}"
    else
        echo ""
        log_info "Benchmark completed"
        
        if [ $failed -eq 0 ]; then
            log_success "All benchmarks passed"
        else
            log_error "Some benchmarks failed"
        fi
    fi
    
    exit $failed
}

main "$@"