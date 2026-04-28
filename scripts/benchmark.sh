#!/bin/bash
# FullStackHex Shared Benchmark Library
# Common functions for all benchmark tools

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"
source "$SCRIPT_DIR/config.sh"

BENCH_BACKEND="${BENCH_BACKEND:-ab}"

benchmark_init() {
    if [ "$JSON_OUTPUT" = false ]; then
        log_info "FullStackHex Performance Benchmark System"
        log_info "Backend: $BENCH_BACKEND"
        echo ""
    fi
}

check_benchmark_deps() {
    local missing=0
    
    if ! command -v curl &> /dev/null; then
        log_error "curl not found"
        missing=1
    else
        log_success "curl found"
    fi
    
    if [ $missing -eq 1 ]; then
        exit 1
    fi
    
    return 0
}

get_bench_requests() {
    echo "${BENCHLITE_REQUESTS:-1000}"
}

get_bench_concurrent() {
    echo "${BENCHLITE_CONCURRENT:-100}"
}

format_latency_result() {
    local name="$1"
    local url="$2"
    local p50="$3"
    local p99="$4"
    local expected_p50="$5"
    local expected_p99="$6"
    
    log_info "Results:"
    log_info "  p50: ${p50}ms (target: <${expected_p50}ms)"
    log_info "  p99: ${p99}ms (target: <${expected_p99}ms)"
    
    local p50_int=$(echo "$p50" | cut -d. -f1)
    local p99_int=$(echo "$p99" | cut -d. -f1)
    local expected_p50_int=$(echo "$expected_p50" | cut -d. -f1)
    local expected_p99_int=$(echo "$expected_p99" | cut -d. -f1)
    
    local passed=0
    local p50_passed=0
    local p99_passed=0
    
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
    
    echo "{\"name\":\"$name\",\"url\":\"$url\",\"p50_ms\":$p50,\"p99_ms\":$p99,\"p50_target_ms\":$expected_p50,\"p99_target_ms\":$expected_p99,\"p50_passed\":$p50_passed,\"p99_passed\":$p99_passed,\"passed\":$[ $passed -eq 0 ]}"
    
    return $passed
}

benchmark_ttfb() {
    local name="$1"
    local url="$2"
    local expected_ms="$3"
    
    log_info "Benchmark: $name"
    log_info "URL: $url"
    echo ""
    
    local ttfb
    ttfb=$(curl -w "%{time_starttransfer}" -o /dev/null -s "$url")
    
    local expected_s=$(echo "scale=3; $expected_ms / 1000" | bc)
    
    log_info "TTFB: ${ttfb}s (target: <${expected_s}s)"
    
    local passed
    passed=$(echo "$ttfb < $expected_s" | bc -l 2>/dev/null || echo "0")
    
    echo "{\"name\":\"$name\",\"url\":\"$url\",\"ttfb_s\":$ttfb,\"ttfb_target_s\":$expected_s,\"passed\":$[ $passed -eq 1 ]}"
    
    if [ "$passed" = "1" ]; then
        log_success "TTFB PASSED"
        return 0
    else
        log_error "TTFB FAILED"
        return 1
    fi
}

output_json_results() {
    local timestamp="$1"
    local git_commit="$2"
    shift 2
    local results=("$@")
    
    echo "{"
    echo "  \"timestamp\": \"$timestamp\","
    echo "  \"git_commit\": \"$git_commit\","
    echo "  \"backend\": \"$BENCH_BACKEND\","
    echo "  \"benchmarks\": ["
    
    local first=true
    for result in "${results[@]}"; do
        if [ "$first" = true ]; then
            first=false
        else
            echo ","
        fi
        echo -n "    $result"
    done
    
    echo ""
    echo "  ]"
    echo "}"
}