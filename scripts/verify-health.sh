#!/bin/bash
set -e

# FullStackHex Health Check Script
# Usage: ./scripts/verify-health.sh [--timeout <seconds>] [--verbose]
# Returns: 0 if all services healthy, 1 otherwise

# Source common functions and configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"
source "$SCRIPT_DIR/config.sh"

# Default values
TIMEOUT=30
VERBOSE=false
FAILED=0

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --timeout)
            TIMEOUT="$2"
            shift 2
            ;;
        --verbose|-v)
            VERBOSE=true
            shift
            ;;
        --help|-h)
            echo "Usage: $0 [--timeout <seconds>] [--verbose]"
            echo ""
            echo "Options:"
            echo "  --timeout <seconds>  Timeout for service checks (default: 30)"
            echo "  --verbose, -v        Enable verbose output"
            echo "  --help, -h           Show this help message"
            exit 0
            ;;
        *)
            log_error "Unknown argument: $1"
            echo "Usage: $0 [--timeout <seconds>] [--verbose]"
            exit 1
            ;;
    esac
done

log_info "FullStackHex - Health Check"

# Check jq is available (needed for baseline.sh)
if ! command -v jq &> /dev/null; then
    log_warning "jq not found - some scripts may not work"
    log_info "Install: sudo apt-get install jq"
fi

# Load environment variables
load_env

# Service configurations (URLs used as reference; PostgreSQL and Redis
# are checked via dedicated CLI tools, not HTTP)
declare -A SERVICES
SERVICES["Rust Backend"]="${RUST_BACKEND_URL:-http://localhost:8001}/health"
SERVICES["Frontend"]="${FRONTEND_URL:-http://localhost:4321}"
SERVICES["PostgreSQL"]="postgresql://${POSTGRES_USER:-app_user}@localhost:${POSTGRES_PORT:-5432}/${POSTGRES_DB:-app_database}"
SERVICES["Redis"]="redis://localhost:${REDIS_PORT:-6379}"

# Check all services
log_info "Checking services (timeout: ${TIMEOUT}s)..."
echo ""

for service in "Rust Backend" "Frontend" "PostgreSQL" "Redis"; do
    case "$service" in
        "PostgreSQL")
            if ! check_postgres; then
                FAILED=1
                log_warning "Start with: docker compose -f compose/dev.yml up -d postgres"
            fi
            ;;
        "Redis")
            if ! check_redis; then
                FAILED=1
                log_warning "Start with: docker compose -f compose/dev.yml up -d redis"
            fi
            ;;
        *)
            url="${SERVICES[$service]}"
            if ! check_service_http "$service" "$url" "$TIMEOUT" "$VERBOSE"; then
                FAILED=1
                case "$service" in
                    "Rust Backend")
                        log_warning "Start with: cd backend && cargo run -p api"
                        ;;
                    "Frontend")
                        log_warning "Start with: cd frontend && bun run dev"
                        ;;
                esac
            fi
            ;;
    esac
    echo ""
done

# Summary
log_info "Health check completed"
if [ $FAILED -eq 0 ]; then
    log_success "All services are healthy"
else
    log_error "Some services are down"
fi

exit $FAILED
