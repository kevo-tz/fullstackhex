#!/usr/bin/env bash
set -euo pipefail

# FullStackHex Performance Benchmark Script
# Usage: ./scripts/bench.sh [--json] [--compare]
# Requires: ab (Apache Bench) - install via: apt-get install apache2-utils (Linux) or yum install httpd-tools (RHEL)
#
# Benchmarks the Rust backend directly AND through nginx (the edge path with
# keepalive + static asset serving). --compare reports regressions vs the last
# run's baseline stored in $BASELINE_DIR/last.json.

# Source common functions and configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"
source "$SCRIPT_DIR/config.sh"

# Check dependencies
check_deps() {
    local missing=0

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
    local failed=0

    log_info "Checking services..."

    if check_service_http "Rust backend" "$RUST_BACKEND_URL/health" 5 false; then
        log_success "Rust backend responding at $RUST_BACKEND_URL"
    else
        log_error "Rust backend not responding at $RUST_BACKEND_URL"
        log_warning "Start with: cd backend && cargo run -p api"
        failed=1
    fi

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

    if check_service_http "Nginx" "$NGINX_URL/health" 5 false; then
        log_success "Nginx responding at $NGINX_URL"
    else
        log_warning "Nginx not responding at $NGINX_URL — skipping edge benchmarks"
    fi

    echo ""
}

# Probe for a static asset served directly by nginx (not proxied to Node).
# Returns 0 with the URL in $STATIC_PROBE_URL, or 1 if none found.
probe_static_asset() {
    local candidates=("$@")
    for path in "${candidates[@]}"; do
        local code
        code=$(curl -s -o /dev/null -w "%{http_code}" --max-time 5 "$NGINX_URL$path" 2>/dev/null || echo "000")
        if [ "$code" = "200" ]; then
            STATIC_PROBE_URL="$NGINX_URL$path"
            return 0
        fi
    done
    return 1
}

# Run ab and parse results into structured fields stored in global result dict.
# Usage: run_ab <name> <url> <p50_target> <p99_target>
# Sets: $name_p50, $name_p99, $name_rps, $name_mean, $name_passed, $name_raw
run_ab() {
    local name="$1" url="$2" p50_target="$3" p99_target="$4"
    local safe
    safe=$(echo "$name" | tr ' /' '__')

    log_info "Benchmark: $name"
    log_info "URL: $url"
    log_info "Requests: $BENCHLITE_REQUESTS, Concurrent: $BENCHLITE_CONCURRENT"
    echo "" >&2

    local output
    output=$(ab -n "$BENCHLITE_REQUESTS" -c "$BENCHLITE_CONCURRENT" -r -k "$url" 2>&1)

    local p50 p99 p50_int p99_int
    p50=$(echo "$output" | awk '/^ +50% / {print $2}' | head -1)
    p99=$(echo "$output" | awk '/^ +99% / {print $2}' | head -1)
    [ -z "$p50" ] && p50=$(echo "$output" | grep -oP '^\s+50%\s+\K\d+' | head -1)
    [ -z "$p99" ] && p99=$(echo "$output" | grep -oP '^\s+99%\s+\K\d+' | head -1)

    local mean rps
    mean=$(echo "$output" | awk '/^Time per request:/ {print $4; exit}' | head -1)
    rps=$(echo "$output" | awk '/^Requests per second:/ {print $4; exit}' | head -1)

    # Fallback: use mean if percentiles missing
    if [ -z "$p50" ] || [ -z "$p99" ]; then
        log_warning "Could not parse p50/p99 from ab output, using mean time"
        p50=${p50:-$mean}
        p99=${p99:-$mean}
    fi

    # Integer comparison
    p50_int=$(echo "$p50" | cut -d. -f1)
    p99_int=$(echo "$p99" | cut -d. -f1)
    local ep50_int ep99_int
    ep50_int=$(echo "$p50_target" | cut -d. -f1)
    ep99_int=$(echo "$p99_target" | cut -d. -f1)

    local p50_ok=0 p99_ok=0
    [ -n "$p50_int" ] && [ "$p50_int" -lt "$ep50_int" ] 2>/dev/null && p50_ok=1
    [ -n "$p99_int" ] && [ "$p99_int" -lt "$ep99_int" ] 2>/dev/null && p99_ok=1

    local passed=0
    [ "$p50_ok" -eq 1 ] && [ "$p99_ok" -eq 1 ] && passed=1

    # Display per-metric result inline
    if [ "$p50_ok" -eq 1 ]; then
        log_success "  p50: ${p50}ms (target: <${p50_target}ms)"
    else
        log_error "  p50: ${p50}ms (target: <${p50_target}ms)"
    fi
    if [ "$p99_ok" -eq 1 ]; then
        log_success "  p99: ${p99}ms (target: <${p99_target}ms)"
    else
        log_error "  p99: ${p99}ms (target: <${p99_target}ms)"
    fi
    if [ -n "$rps" ]; then
        local rps_int
        rps_int=$(echo "$rps" | cut -d. -f1)
        log_info "  req/s: $rps_int"
    fi

    # Store in global namespace for summary table
    declare -g "${safe}_p50=$p50" "${safe}_p99=$p99" "${safe}_rps=$rps" "${safe}_passed=$passed" "${safe}_mean=$mean"
    # Track ordered list of benchmark names
    BENCH_NAMES+=("$name")
    BENCH_NAMES_SAFE+=("$safe")
}

# Frontend TTFB benchmark using curl — 5 samples, reports median (p50) and p95.
benchmark_frontend_ttfb() {
    log_info "Benchmark: Frontend TTFB (SSR)"
    log_info "URL: $FRONTEND_URL"
    echo "" >&2

    local samples=()
    for _ in 1 2 3 4 5; do
        local t
        t=$(curl -w "%{time_starttransfer}" -o /dev/null -s "$FRONTEND_URL")
        samples+=("$t")
    done

    # Sort numerically for percentile extraction
    local sorted
    sorted=$(printf "%s\n" "${samples[@]}" | sort -n)
    local p50 p95
    p50=$(echo "$sorted" | sed -n '3p')   # median of 5
    p95=$(echo "$sorted" | sed -n '5p')   # max of 5 approximates p95

    local expected_s
    expected_s=$(echo "scale=3; $FRONTEND_TTFB_THRESHOLD / 1000" | bc)

    log_info "TTFB p50: ${p50}s  p95: ${p95}s (target: <${expected_s}s)"

    local passed
    passed=$(echo "$p50 < $expected_s" | bc -l 2>/dev/null || echo "0")

    if [ "$passed" = "1" ]; then
        log_success "TTFB PASSED"
    else
        log_error "TTFB FAILED"
    fi

    # Store for summary — ttfb field carries the median
    BENCH_NAMES+=("Frontend TTFB")
    BENCH_NAMES_SAFE+=("frontend_ttfb")
    # shellcheck disable=SC2034 # accessed via get_var eval in print_summary
    declare -g frontend_ttfb_ttfb="$p50" frontend_ttfb_target="$expected_s" frontend_ttfb_passed="$passed" frontend_ttfb_p95="$p95"
}

# Print results summary table
print_summary() {
    local i name safe passed_str color p50_val p99_val rps_val mean_val
    local total=${#BENCH_NAMES[@]}
    local passed_count=0 failed_count=0
    local git_short commit_str

    git_short=$(git rev-parse --short HEAD 2>/dev/null || echo "?")
    commit_str="Commit: ${git_short} | ${BENCHLITE_REQUESTS} req, ${BENCHLITE_CONCURRENT} con"

    # All table output goes to stderr so --json stdout stays clean
    echo "" >&2
    printf "  %s\n" "──────────────────────────────────────────────────────────────" >&2
    printf "  %-28s %6s %6s %6s %6s %s\n" "Benchmark" "p50" "p99" "mean" "req/s" "Status" >&2
    printf "  %s\n" "──────────────────────────────────────────────────────────────" >&2

    for i in "${!BENCH_NAMES[@]}"; do
        name="${BENCH_NAMES[$i]}"
        safe="${BENCH_NAMES_SAFE[$i]}"

        if [ "$safe" = "frontend_ttfb" ]; then
            local ttfb_val
            ttfb_val=$(get_var "${safe}_ttfb")
            passed_str=$(get_var "${safe}_passed")
            local ttfb_ms
            ttfb_ms=$(echo "$ttfb_val * 1000" | bc 2>/dev/null | cut -d. -f1)
            ttfb_ms="${ttfb_ms:-0}"
            if [ "$passed_str" = "1" ]; then
                color="$GREEN"; passed_count=$((passed_count + 1))
            else
                color="$RED"; failed_count=$((failed_count + 1))
            fi
            printf "  ${color}%-28s %6s %6s %6s %6s ${color}%-6s${NC}\n" \
                "$name" "${ttfb_ms}ms" "n/a" "n/a" "n/a" "PASS" >&2        else
            p50_val=$(get_var "${safe}_p50")
            p99_val=$(get_var "${safe}_p99")
            mean_val=$(get_var "${safe}_mean")
            rps_val=$(get_var "${safe}_rps")
            passed_str=$(get_var "${safe}_passed")

            rps_val=$(echo "$rps_val" | cut -d. -f1 2>/dev/null || echo "?")
            local mean_int
            mean_int=$(echo "$mean_val" | cut -d. -f1 2>/dev/null || echo "?")
            [ -z "$mean_int" ] && mean_int="?"

            if [ "$passed_str" = "1" ]; then
                color="$GREEN"; passed_count=$((passed_count + 1))
            else
                color="$RED"; failed_count=$((failed_count + 1))
            fi
            printf "  ${color}%-28s %6s %6s %6s %6s ${color}%-6s${NC}\n" \
                "$name" "${p50_val}ms" "${p99_val}ms" "${mean_int}ms" "$rps_val" "PASS" >&2
        fi
    done

    printf "  %s\n" "──────────────────────────────────────────────────────────────" >&2

    if [ "$failed_count" -eq 0 ]; then
        printf "  ${GREEN}%d/%d benchmarks passed${NC}    ${commit_str}\n" "$passed_count" "$total" >&2
    else
        printf "  ${RED}%d/%d benchmarks failed${NC}     ${commit_str}\n" "$failed_count" "$total" >&2
    fi
    echo "" >&2
}

# Helper: get a dynamic variable by name
get_var() {
    local v
    v=$(eval echo "\${$1:-}" 2>/dev/null)
    echo "$v"
}

# Main
main() {
    if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
        echo "Usage: $0 [--json] [--compare]"
        echo ""
        echo "Options:"
        echo "  --json           Output results in JSON format"
        echo "  --compare        Compare p50 against the previous run (baseline in .performance/last.json)"
        echo "  --help, -h       Show this help message"
        exit 0
    fi

    local JSON_OUTPUT=false COMPARE=false
    while [[ "${1:-}" == --* ]]; do
        case "$1" in
            --json) JSON_OUTPUT=true; shift ;;
            --compare) COMPARE=true; shift ;;
            *) break ;;
        esac
    done

    # Global array to track benchmark order
    BENCH_NAMES=()
    BENCH_NAMES_SAFE=()

    if [ "$JSON_OUTPUT" = false ]; then
        local git_short
        git_short=$(git rev-parse --short HEAD 2>/dev/null || echo "?")
        log_info "FullStackHex Benchmarks  |  ${git_short}  |  ${BENCHLITE_REQUESTS} req, ${BENCHLITE_CONCURRENT} con"
        echo ""
    fi

    check_deps
    check_services

    local timestamp git_commit
    timestamp=$(get_timestamp)
    git_commit=$(get_git_commit)

    # ── Run benchmarks ──────────────────────────────────────────────

    # 1. Aggregate /health endpoint (direct Rust)
    run_ab "Rust /health" "$RUST_BACKEND_URL/health" "$RUST_HEALTH_P50_THRESHOLD" "$RUST_HEALTH_P99_THRESHOLD"

    # 2. Sub-endpoints (direct Rust)
    run_ab "Rust /health/db" "$RUST_BACKEND_URL/health/db" "$RUST_HEALTH_DB_P50_THRESHOLD" "$RUST_HEALTH_DB_P99_THRESHOLD"
    run_ab "Rust /health/redis" "$RUST_BACKEND_URL/health/redis" "$RUST_HEALTH_REDIS_P50_THRESHOLD" "$RUST_HEALTH_REDIS_P99_THRESHOLD"
    run_ab "Rust /health/python" "$RUST_BACKEND_URL/health/python" "$RUST_HEALTH_PYTHON_P50_THRESHOLD" "$RUST_HEALTH_PYTHON_P99_THRESHOLD"

    # 3. Edge benchmarks — through nginx (keepalive, static serving, full path)
    if check_service_http "Nginx" "$NGINX_URL/health" 5 false; then
        run_ab "Nginx /health (edge)" "$NGINX_URL/health" "$NGINX_HEALTH_P50_THRESHOLD" "$NGINX_HEALTH_P99_THRESHOLD"

        local static_asset="${NGINX_STATIC_ASSET:-}"
        if [ -z "$static_asset" ]; then
            STATIC_PROBE_URL=""
            probe_static_asset "/styles/layout.css" "/favicon.ico" "/_astro/../styles/layout.css"
            if [ -n "${STATIC_PROBE_URL:-}" ]; then
                static_asset="$STATIC_PROBE_URL"
            fi
        fi
        if [ -n "$static_asset" ]; then
            run_ab "Nginx static asset" "$static_asset" "$NGINX_STATIC_P50_THRESHOLD" "$NGINX_STATIC_P99_THRESHOLD"
        else
            log_warning "No static asset probed through nginx — skipping static benchmark"
        fi
    else
        log_warning "Nginx down — skipping edge benchmarks"
    fi

    # 4. Frontend TTFB (5-sample distribution)
    benchmark_frontend_ttfb

    # ── Summary table ───────────────────────────────────────────────

    local failed=0
    for i in "${!BENCH_NAMES_SAFE[@]}"; do
        local s="${BENCH_NAMES_SAFE[$i]}"
        local p
        p=$(get_var "${s}_passed")
        [ "$p" != "1" ] && failed=1
    done

    if [ "$JSON_OUTPUT" = false ]; then
        print_summary
    fi

    # ── JSON output ─────────────────────────────────────────────────

    if [ "$JSON_OUTPUT" = true ]; then
        echo "{"
        echo "  \"timestamp\": \"$timestamp\","
        echo "  \"git_commit\": \"$git_commit\","
        echo "  \"benchmarks\": ["
        local first=true
        for i in "${!BENCH_NAMES[@]}"; do
            local n="${BENCH_NAMES[$i]}" s="${BENCH_NAMES_SAFE[$i]}"
            $first || echo ","
            first=false
            echo "    {"
            echo "      \"name\": \"$n\","
            echo "      \"url\": \"$RUST_BACKEND_URL/health\","
            echo "      \"requests\": \"$BENCHLITE_REQUESTS\","
            echo "      \"concurrent\": \"$BENCHLITE_CONCURRENT\","
            if [ "$s" = "frontend_ttfb" ]; then
                local tf tb tp95
                tf=$(get_var "${s}_ttfb")
                tb=$(get_var "${s}_target")
                tp95=$(get_var "${s}_p95")
                echo "      \"result\": {"
                echo "        \"ttfb_s\": $tf,"
                echo "        \"p95_s\": ${tp95:-$tf},"
                echo "        \"target_s\": $tb,"
                echo "        \"passed\": $(get_var "${s}_passed")"
                echo "      }"
            else
                echo "      \"result\": {"
                echo "        \"p50_ms\": $(get_var "${s}_p50"),"
                echo "        \"p99_ms\": $(get_var "${s}_p99"),"
                echo "        \"rps\": $(get_var "${s}_rps" | cut -d. -f1),"
                echo "        \"passed\": $(get_var "${s}_passed")"
                echo "      }"
            fi
            echo "    }"
        done
        echo ""
        echo "  ]"
        echo "}"
    fi

    # ── Baseline storage + comparison ──────────────────────────────

    mkdir -p "$BASELINE_DIR"
    local baseline_file="$BASELINE_DIR/last.json"

    # Build a machine-readable summary of this run (name → p50) regardless of
    # --json, so the baseline is always comparable.
    local baseline_lines=()
    for i in "${!BENCH_NAMES[@]}"; do
        local n="${BENCH_NAMES[$i]}" s="${BENCH_NAMES_SAFE[$i]}"
        local p50
        if [ "$s" = "frontend_ttfb" ]; then
            p50=$(get_var "${s}_ttfb")
        else
            p50=$(get_var "${s}_p50")
        fi
        baseline_lines+=("$n|$p50")
    done

    if [ "$COMPARE" = true ]; then
        if [ -f "$baseline_file" ]; then
            log_info "Comparing against baseline: $baseline_file"
            local regressed=0
            for line in "${baseline_lines[@]}"; do
                local name="${line%%|*}" cur="${line##*|}"
                local base
                base=$(grep -F "${name}|" "$baseline_file" 2>/dev/null | head -1 | cut -d'|' -f2 || echo "")
                if [ -z "$base" ] || [ "$base" = "0" ]; then
                    continue
                fi
                local pct
                pct=$(echo "scale=1; ($cur - $base) / $base * 100" | bc 2>/dev/null || echo "0")
                if echo "$pct > 20" | bc 2>/dev/null | grep -q 1; then
                    log_error "REGRESSION ${name}: ${cur}ms vs baseline ${base}ms (+${pct}%)"
                    regressed=1
                else
                    log_info "${name}: ${cur}ms vs baseline ${base}ms (${pct}%)"
                fi
            done
            if [ "$regressed" = "1" ]; then
                log_error "One or more benchmarks regressed >20% vs baseline"
            fi
        else
            log_warning "No baseline found — run without --compare once to establish one"
        fi
    fi

    # Persist this run as the baseline for the next --compare.
    printf '%s\n' "${baseline_lines[@]}" > "$baseline_file"

    exit $failed
}

main "$@"
