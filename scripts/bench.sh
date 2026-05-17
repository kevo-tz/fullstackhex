#!/usr/bin/env bash
set -euo pipefail

# FullStackHex Performance Benchmark Script
# Usage: ./scripts/bench.sh [--json]
# Requires: ab (Apache Bench) - install via: apt-get install apache2-utils (Linux) or yum install httpd-tools (RHEL)

# Source common functions and configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"
source "$SCRIPT_DIR/config.sh"

# Configuration is now sourced from config.sh

# Check dependencies
check_deps() {
    local missing
    missing=0;
  
    if ! command -v ab &> /dev/null; then
        log_error "ab (Apache Bench) not found"
        log_info "Install:"
        log_info "  Linux (Debian/Ubuntu): sudo apt-get install apache2-utils"
        log_info "  Linux (RHEL/CentOS): sudo yum install httpd-tools"
        log_info "  macOS: ab is included with Apache (or brew install httpd)"
        missing=1
    else
        local version
        version=$(ab -V 2>&1 | head -1)
        log_success "ab found: $version"
    fi
  
    if ! command -v curl &> /dev/null; then
        log_error "curl not found"
        missing=1
    else
        log_success "curl found"
    fi

    if ! command -v bc &> /dev/null; then
        log_error "bc not found"
        log_info "Install:"
        log_info "  Linux (Debian/Ubuntu): sudo apt-get install bc"
        log_info "  Linux (RHEL/CentOS): sudo yum install bc"
        log_info "  macOS: brew install bc"
        missing=1
    else
        log_success "bc found"
    fi
  
    if [ $missing -eq 1 ]; then
        exit 1
    fi
}

# Check if services are running
check_services() {
    local failed
    failed=0
 
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

# Benchmark function using ab
benchmark() {
    local name
    name="$1"
    local url
    url="$2"
    local expected_p50
    expected_p50="$3"
    local expected_p99
    expected_p99="$4"
    
    log_info "Benchmark: $name"
    log_info "URL: $url"
    log_info "Requests: $BENCHLITE_REQUESTS, Concurrent: $BENCHLITE_CONCURRENT"
    echo "" >&2

    # Run ab and capture output
    local output
    output=$(ab -n "$BENCHLITE_REQUESTS" -c "$BENCHLITE_CONCURRENT" -r -k "$url" 2>&1)
    
    # Parse p50 and p99 from "Percentage of requests served within a certain time" table
    # Format: "  50%    123" (ms)
    local p50
    p50=$(echo "$output" | awk '/^ +50% / {print $2}' | head -1)
    local p99
    p99=$(echo "$output" | awk '/^ +99% / {print $2}' | head -1)
    
    # If p50/p99 not found, try alternative format
    if [ -z "$p50" ]; then
        p50=$(echo "$output" | grep -oP '^\s+50%\s+\K\d+' | head -1)
    fi
    if [ -z "$p99" ]; then
        p99=$(echo "$output" | grep -oP '^\s+99%\s+\K\d+' | head -1)
    fi
    
    # Fallback: use mean time if percentiles not available
    if [ -z "$p50" ] || [ -z "$p99" ]; then
        log_warning "Could not parse p50/p99 from ab output, using mean time"
        local mean
        mean=$(echo "$output" | awk '/^Time per request:/ {print $4; exit}' | head -1)
        p50=${p50:-$mean}
        p99=${p99:-$mean}
    fi
    
    log_info "Results:"
    log_info "  p50: ${p50}ms (target: <${expected_p50}ms)"
    log_info "  p99: ${p99}ms (target: <${expected_p99}ms)"
    
    # Check against targets (convert to integers for comparison)
    local p50_int
    p50_int=$(echo "$p50" | cut -d. -f1)
    local p99_int
    p99_int=$(echo "$p99" | cut -d. -f1)
    local expected_p50_int
    expected_p50_int=$(echo "$expected_p50" | cut -d. -f1)
    local expected_p99_int
    expected_p99_int=$(echo "$expected_p99" | cut -d. -f1)
    
    local passed
    passed=0
    
    local p50_passed
    p50_passed=0
    local p99_passed
    p99_passed=0
    
    if [ -n "$p50_int" ] && [ "$p50_int" -lt "$expected_p50_int" ] 2>/dev/null; then
        log_success "p50 PASSED"
        p50_passed=1
    else
        log_error "p50 FAILED"
        passed=1
    fi
    
    if [ -n "$p99_int" ] && [ "$p99_int" -lt "$expected_p99_int" ] 2>/dev/null; then
        log_success "p99 PASSED"
        p99_passed=1
    else
        log_error "p99 FAILED"
        passed=1
    fi
    
    # Return structured result for JSON output
    echo "{\"name\":\"$name\",\"url\":\"$url\",\"p50_ms\":$p50,\"p99_ms\":$p99,\"p50_target_ms\":$expected_p50,\"p99_target_ms\":$expected_p99,\"p50_passed\":$p50_passed,\"p99_passed\":$p99_passed,\"passed\":$(( passed == 0 ))}"
    
    return $passed
}

# Frontend TTFB benchmark using curl
benchmark_frontend_ttfb() {
    log_info "Benchmark: Frontend TTFB (SSR)"
    log_info "URL: $FRONTEND_URL"
    echo "" >&2

    local ttfb
    ttfb=$(curl -w "%{time_starttransfer}" -o /dev/null -s "$FRONTEND_URL")
    local expected_s
    expected_s=$(echo "scale=3; $FRONTEND_TTFB_THRESHOLD / 1000" | bc)
    
    log_info "TTFB: ${ttfb}s (target: <${expected_s}s)"
    
    local passed
    passed=$(echo "$ttfb < $expected_s" | bc -l 2>/dev/null || echo "0")
    
    # Return structured result for JSON output
    echo "{\"name\":\"Frontend TTFB\",\"url\":\"$FRONTEND_URL\",\"ttfb_s\":$ttfb,\"ttfb_target_s\":$expected_s,\"passed\":$passed}"
    
    if [ "$passed" = "1" ]; then
        log_success "TTFB PASSED"
        return 0
    else
        log_error "TTFB FAILED"
        return 1
    fi
}

# Main
main() {
    # Check for --help flag
    if [[ "$1" == "--help" || "$1" == "-h" ]]; then
        echo "Usage: $0 [--json] [--compare]"
        echo ""
        echo "Options:"
        echo "  --json           Output results in JSON format"
        echo "  --compare        Compare against baseline (non-blocking warning)"
        echo "  --help, -h       Show this help message"
        exit 0
    fi
    
    # Check for JSON output flag
    local JSON_OUTPUT
    JSON_OUTPUT=false
    local COMPARE
    COMPARE=false
    while [[ "$1" == --* ]]; do
        case "$1" in
            --json) JSON_OUTPUT=true; shift ;;
            --compare) COMPARE=true; shift ;;
            *) break ;;
        esac
    done

    if [ "$JSON_OUTPUT" = false ]; then
        log_info "FullStackHex Performance Benchmarks (Lite - using Apache Bench)"
        echo ""
    fi

    check_deps
    check_services
    
    if [ "$JSON_OUTPUT" = false ]; then
        log_info "Configuration:"
        log_info "  RUST_BACKEND_URL: $RUST_BACKEND_URL"
        log_info "  FRONTEND_URL: $FRONTEND_URL"
log_info "  Requests: $BENCHLITE_REQUESTS"
    log_info "  Concurrent: $BENCHLITE_CONCURRENT"
    fi
    
    # Initialize results for JSON output
    local timestamp
    timestamp=$(get_timestamp)
    local git_commit
    git_commit=$(get_git_commit)
    
    local failed
    failed=0
    
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
    
    # Baseline comparison (non-blocking warning)
    if [ "$COMPARE" = true ] && [ -f benches/baseline/baseline.json ]; then
        log_info "Comparing against baseline..."
        local baseline_p99
        baseline_p99=$(jq -r '.benchmarks[0].result.p99_ms' benches/baseline/baseline.json 2>/dev/null || echo "0")
        local current_p99
        current_p99=$(echo "$health_result" | jq -r '.p99_ms' 2>/dev/null || echo "0")
        if [ -n "$baseline_p99" ] && [ -n "$current_p99" ] && [ "$baseline_p99" != "0" ]; then
            local delta
            delta=$(echo "scale=1; (($current_p99 - $baseline_p99) / $baseline_p99) * 100" | bc -l 2>/dev/null || echo "0")
            if [ "${delta%.*}" -gt 20 ] 2>/dev/null; then
                log_warning "p99 regression detected: ${delta}% slower than baseline (baseline: ${baseline_p99}ms, current: ${current_p99}ms)"
            else
                log_success "p99 within baseline tolerance (${delta}% delta)"
            fi
        fi
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
        echo "      \"requests\": \"$BENCHLITE_REQUESTS\","
        echo "      \"concurrent\": \"$BENCHLITE_CONCURRENT\","
        echo "      \"result\": $health_result"
        echo "    },"
        echo "    {"
        echo "      \"name\": \"Frontend TTFB\","
        echo "      \"url\": \"$FRONTEND_URL\","
        echo "      \"result\": $frontend_result"
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
