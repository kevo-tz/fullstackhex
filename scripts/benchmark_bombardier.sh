#!/bin/bash
# FullStackHex Benchmark Adapter - Bombardier
# Usage: source this file to use bombardier as benchmark backend

check_bombardier_dependencies() {
    local missing=0
    
    if ! command -v bombardier &> /dev/null; then
        log_error "bombardier not found"
        log_info "Install:"
        log_info "  Linux/macOS: go install github.com/c0n水平的acker/bombardier@latest"
        log_info "  Then add $HOME/go/bin to your PATH"
        missing=1
    else
        local version
        version=$(bombardier -V 2>&1 | head -1)
        log_success "bombardier found: $version"
    fi
    
    if [ $missing -eq 1 ]; then
        return 1
    fi
    
    return 0
}

run_benchmark_bombardier() {
    local name="$1"
    local url="$2"
    local duration="$3"
    local concurrent="$4"
    local expected_p50="$5"
    local expected_p99="$6"
    
    log_info "Benchmark: $name"
    log_info "URL: $url"
    log_info "Duration: $duration, Concurrent: $concurrent"
    echo ""
    
    local output
    output=$(bombardier -c "$concurrent" -d "$duration" "$url" 2>&1)
    
    local p50
    p50=$(echo "$output" | awk -F', ' '/p50/ {gsub(/[^0-9.]/,"",$2); print $2; exit}')
    local p99
    p99=$(echo "$output" | awk -F', ' '/p99/ {gsub(/[^0-9.]/,"",$2); print $2; exit}')
    
    if [ -z "$p50" ]; then
        p50=$(echo "$output" | grep -oP 'p50.*?(\d+\.?\d*)\s*ms' | grep -oP '\d+\.?\d*' | head -1)
    fi
    if [ -z "$p99" ]; then
        p99=$(echo "$output" | grep -oP 'p99.*?(\d+\.?\d*)\s*ms' | grep -oP '\d+\.?\d*' | head -1)
    fi
    
    if [ -z "$p50" ] || [ -z "$p99" ]; then
        log_warning "Could not parse p50/p99 from bombardier output"
        p50=${p50:-0}
        p99=${p99:-0}
    fi
    
    format_latency_result "$name" "$url" "$p50" "$p99" "$expected_p50" "$expected_p99"
    
    local p50_int=$(echo "$p50" | cut -d. -f1)
    local p99_int=$(echo "$p99" | cut -d. -f1)
    local expected_p50_int=$(echo "$expected_p50" | cut -d. -f1)
    local expected_p99_int=$(echo "$expected_p99" | cut -d. -f1)
    
    if [ -n "$p50_int" ] && [ "$p50_int" -lt "$expected_p50_int" ] 2>/dev/null; then
        return 0
    else
        return 1
    fi
}