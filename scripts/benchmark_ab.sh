#!/bin/bash
# FullStackHex Benchmark Adapter - Apache Bench (ab)
# Usage: source this file to use ab as benchmark backend

check_ab_dependencies() {
    local missing=0
    
    if ! command -v ab &> /dev/null; then
        log_error "ab (Apache Bench) not found"
        log_info "Install:"
        log_info "  Linux (Debian/Ubuntu): sudo apt-get install apache2-utils"
        log_info "  Linux (RHEL/CentOS): sudo yum install httpd-tools"
        log_info "  macOS: brew install httpd"
        missing=1
    else
        local version
        version=$(ab -V 2>&1 | head -1)
        log_success "ab found: $version"
    fi
    
    if [ $missing -eq 1 ]; then
        return 1
    fi
    
    return 0
}

run_benchmark_ab() {
    local name="$1"
    local url="$2"
    local requests="$3"
    local concurrent="$4"
    local expected_p50="$5"
    local expected_p99="$6"
    
    log_info "Benchmark: $name"
    log_info "URL: $url"
    log_info "Requests: $requests, Concurrent: $concurrent"
    echo ""
    
    local output
    output=$(ab -n "$requests" -c "$concurrent" -r -k "$url" 2>&1)
    
    local p50
    p50=$(echo "$output" | awk '/^ +50% / {print $2}' | head -1)
    local p99
    p99=$(echo "$output" | awk '/^ +99% / {print $2}' | head -1)
    
    if [ -z "$p50" ]; then
        p50=$(echo "$output" | grep -oP '^\s+50%\s+\K[0-9]+' | head -1)
    fi
    if [ -z "$p99" ]; then
        p99=$(echo "$output" | grep -oP '^\s+99%\s+\K[0-9]+' | head -1)
    fi
    
    if [ -z "$p50" ] || [ -z "$p99" ]; then
        log_warning "Could not parse p50/p99 from ab output, using mean time"
        local mean
        mean=$(echo "$output" | awk '/^Time per request:/ {print $4; exit}' | head -1)
        p50=${p50:-$mean}
        p99=${p99:-$mean}
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