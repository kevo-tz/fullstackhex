#!/bin/bash
set -e

# FullStackHex Baseline Performance Profiling Script
# Usage: ./scripts/baseline.sh [--save] [--compare]
#   --save      Save current results as new baseline
#   --compare   Compare current results against saved baseline

# Source common functions and configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"
source "$SCRIPT_DIR/config.sh"

# Check dependencies
check_deps() {
    local missing=0

    if ! command -v jq &> /dev/null; then
        log_error "jq not found"
        log_info "Install: sudo apt-get install jq (Debian/Ubuntu) or brew install jq (macOS)"
        missing=1
    else
        log_success "jq found"
    fi

    if [ $missing -eq 1 ]; then
        exit 1
    fi
}

check_deps

# Configuration
BASELINE_DIR=".performance"
BASELINE_FILE="$BASELINE_DIR/baseline.json"
RESULTS_FILE="$BASELINE_DIR/results-$(date +%Y%m%d_%H%M%S).json"
HTML_REPORT="$BASELINE_DIR/report.html"

# Parse arguments
SAVE=false
COMPARE=false

for arg in "$@"; do
    case $arg in
        --save)
            SAVE=true
            ;;
        --compare)
            COMPARE=true
            ;;
        --help|-h)
            echo "Usage: $0 [--save] [--compare]"
            echo ""
            echo "Options:"
            echo "  --save      Save current results as new baseline"
            echo "  --compare   Compare current results against saved baseline"
            echo "  --help, -h  Show this help message"
            exit 0
            ;;
        *)
            log_error "Unknown argument: $arg"
            echo "Usage: $0 [--save] [--compare]"
            exit 1
            ;;
    esac
done

log_info "FullStackHex - Baseline Profiling"
echo ""

# Ensure services are running
log_info "Checking services..."
if ! "$SCRIPT_DIR/verify-health.sh" > /dev/null 2>&1; then
    log_error "Services not healthy. Run: make up"
    exit 1
fi
log_success "Services are healthy"
echo ""

# Run benchmarks and capture results
log_info "Running benchmarks..."
echo ""

# Run bench.sh and capture output
BENCH_OUTPUT=$("$SCRIPT_DIR/bench.sh" 2>&1) || true

echo "$BENCH_OUTPUT"
echo ""

# Parse results
log_info "Parsing results..."

# Create baseline directory if needed
mkdir -p "$BASELINE_DIR"

# Save results (simplified - in real implementation, parse bench.sh output)
log_warning "Results contain placeholder values — real parsing of bench.sh output is not yet implemented"
cat > "$RESULTS_FILE" << EOF
{
  "timestamp": "$(date -Iseconds)",
  "git_commit": "$(git rev-parse HEAD 2>/dev/null || echo 'unknown')",
  "results": {
    "rust_health_p50_ms": 0,
    "rust_health_p99_ms": 0,
    "frontend_ttfb_s": 0
  },
  "raw_output": $(echo "$BENCH_OUTPUT" | jq -R -s '.')
}
EOF

log_success "Results saved to $RESULTS_FILE"

# Save as baseline if requested
if [ "$SAVE" = true ]; then
    cp "$RESULTS_FILE" "$BASELINE_FILE"
    git add "$BASELINE_FILE"
    git commit -m "perf: update performance baseline

Baseline taken at $(date -Iseconds)
Commit: $(git rev-parse HEAD 2>/dev/null || echo 'unknown')" || log_warning "Baseline not committed (no changes or git error)"
    log_success "Baseline saved to $BASELINE_FILE"
fi

# Compare with baseline
if [ "$COMPARE" = true ]; then
    if [ ! -f "$BASELINE_FILE" ]; then
        log_error "No baseline found. Run: $0 --save"
        exit 1
    fi

    log_info "Comparing with baseline..."
    echo ""

    BASELINE_TIME=$(jq -r '.timestamp' "$BASELINE_FILE")
    log_info "Baseline from: $BASELINE_TIME"

    log_success "Comparison complete"
    log_warning "Full comparison requires structured output from bench.sh"
fi

# Generate HTML report
echo ""

# Generate HTML report
log_info "Generating HTML report..."

cat > "$HTML_REPORT" << 'EOF'
<!DOCTYPE html>
<html>
<head>
    <title>FullStackHex Performance Report</title>
    <style>
        body { font-family: Arial, sans-serif; margin: 40px; background: #f5f5f5; }
        h1 { color: #333; }
        .metric { background: white; padding: 20px; margin: 10px 0; border-radius: 5px; box-shadow: 0 2px 4px rgba(0,0,0,0.1); }
        .pass { color: green; }
        .fail { color: red; }
        .value { font-size: 24px; font-weight: bold; }
    </style>
</head>
<body>
    <h1>FullStackHex Performance Report</h1>
    <p>Generated: <span id="timestamp"></span></p>

    <div class="metric">
        <h3>Rust /health - p50 Latency</h3>
        <div class="value" id="p50">-- ms</div>
        <p>Target: < 5ms</p>
    </div>

    <div class="metric">
        <h3>Rust /health - p99 Latency</h3>
        <div class="value" id="p99">-- ms</div>
        <p>Target: < 20ms</p>
    </div>

    <div class="metric">
        <h3>Frontend TTFB</h3>
        <div class="value" id="ttfb">-- s</div>
        <p>Target: < 0.1s</p>
    </div>

    <script>
        document.getElementById('timestamp').textContent = new Date().toISOString();

        // Load results from JSON (in real implementation)
        // fetch('results.json').then(r => r.json()).then(data => {
        //     document.getElementById('p50').textContent = data.results.rust_health_p50_ms + ' ms';
        //     document.getElementById('p99').textContent = data.results.rust_health_p99_ms + ' ms';
        //     document.getElementById('ttfb').textContent = data.results.frontend_ttfb_s + ' s';
        // });
    </script>
</body>
</html>
EOF

log_success "HTML report generated: $HTML_REPORT"
echo ""
log_success "Profiling complete"
echo ""
echo "Files:"
echo "  Results: $RESULTS_FILE"
echo "  Baseline: $BASELINE_FILE (if saved)"
echo "  Report: $HTML_REPORT"
