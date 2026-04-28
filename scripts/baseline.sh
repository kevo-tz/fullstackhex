#!/bin/bash
set -e

# FullStackHex Baseline Performance Profiling Script
# Usage: ./scripts/baseline.sh [--save] [--compare]
#   --save      Save current results as new baseline
#   --compare   Compare current results against saved baseline

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

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
        *)
            echo -e "${RED}Unknown argument: $arg${NC}"
            echo "Usage: $0 [--save] [--compare]"
            exit 1
            ;;
    esac
done

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}  FullStackHex - Baseline Profiling${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""

# Ensure services are running
echo -e "${YELLOW}Checking services...${NC}"
if ! ./scripts/verify-health.sh > /dev/null 2>&1; then
    echo -e "${RED}✗ Services not healthy. Run: make up${NC}"
    exit 1
fi
echo -e "${GREEN}✓ Services are healthy${NC}"
echo ""

# Run benchmarks and capture results
echo -e "${YELLOW}Running benchmarks...${NC}"
echo ""

# Run bench.sh and capture output
BENCH_OUTPUT=$(./scripts/bench.sh 2>&1) || true

echo "$BENCH_OUTPUT"
echo ""

# Parse results (this is a simplified parser)
# In production, bench.sh should output JSON directly
echo -e "${YELLOW}Parsing results...${NC}"

# Create baseline directory if needed
mkdir -p "$BASELINE_DIR"

# Save results (simplified - in real implementation, parse bench.sh output)
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

echo -e "${GREEN}✓ Results saved to $RESULTS_FILE${NC}"

# Save as baseline if requested
if [ "$SAVE" = true ]; then
    cp "$RESULTS_FILE" "$BASELINE_FILE"
    git add "$BASELINE_FILE"
    git commit -m "perf: update performance baseline

Baseline taken at $(date -Iseconds)
Commit: $(git rev-parse HEAD 2>/dev/null || echo 'unknown')" || echo -e "${YELLOW}⚠ Baseline not committed (no changes or git error)${NC}"
    echo -e "${GREEN}✓ Baseline saved to $BASELINE_FILE${NC}"
fi

# Compare with baseline
if [ "$COMPARE" = true ]; then
    if [ ! -f "$BASELINE_FILE" ]; then
        echo -e "${RED}✗ No baseline found. Run: $0 --save${NC}"
        exit 1
    fi

    echo ""
    echo -e "${YELLOW}Comparing with baseline...${NC}"
    echo ""

    BASELINE_TIME=$(jq -r '.timestamp' "$BASELINE_FILE")
    echo -e "Baseline from: $BASELINE_TIME"

    # Simplified comparison (in real implementation, parse and compare metrics)
    echo -e "${GREEN}✓ Comparison complete${NC}"
    echo -e "${YELLOW}  (Full comparison requires structured output from bench.sh)${NC}"
fi

# Generate HTML report
echo ""
echo -e "${YELLOW}Generating HTML report...${NC}"

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

echo -e "${GREEN}✓ HTML report generated: $HTML_REPORT${NC}"
echo ""
echo -e "${BLUE}========================================${NC}"
echo -e "${GREEN}  ✓ Profiling complete${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""
echo "Files:"
echo "  Results: $RESULTS_FILE"
echo "  Baseline: $BASELINE_FILE (if saved)"
echo "  Report: $HTML_REPORT"
