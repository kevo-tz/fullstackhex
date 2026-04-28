#!/bin/bash
set -e

# FullStackHex Health Check Script
# Usage: ./scripts/verify-health.sh [--timeout <seconds>] [--verbose]
# Returns: 0 if all services healthy, 1 otherwise

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

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
        *)
            echo -e "${RED}Unknown argument: $1${NC}"
            echo "Usage: $0 [--timeout <seconds>] [--verbose]"
            exit 1
            ;;
    esac
done

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}  FullStackHex - Health Check${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""

# Source .env if exists
if [ -f .env ]; then
    set -a
    source .env
    set +a
fi

# Service configurations
declare -A SERVICES
SERVICES["Rust Backend"]="${RUST_BACKEND_URL:-http://localhost:8001}/health"
SERVICES["Frontend"]="${FRONTEND_URL:-http://localhost:4321}"
SERVICES["PostgreSQL"]="postgres://${POSTGRES_USER:-app_user}:${POSTGRES_PASSWORD:-CHANGE_ME}@localhost:${POSTGRES_PORT:-5432}/${POSTGRES_DB:-app_database}?sslmode=disable"
SERVICES["Redis"]="redis://localhost:${REDIS_PORT:-6379}"

# Function to check HTTP service
check_http_service() {
    local name="$1"
    local url="$2"
    local start_time=$(date +%s)
    local end_time=$((start_time + TIMEOUT))

    if [ "$VERBOSE" = true ]; then
        echo -e "${YELLOW}Checking $name at $url...${NC}"
    fi

    while [ $(date +%s) -lt $end_time ]; do
        if curl --silent --fail "$url" > /dev/null 2>&1; then
            echo -e "${GREEN}✓ $name is healthy${NC}"
            return 0
        fi
        sleep 1
    done

    echo -e "${RED}✗ $name is not responding${NC}"
    return 1
}

# Function to check PostgreSQL
check_postgres() {
    local name="PostgreSQL"
    local start_time=$(date +%s)
    local end_time=$((start_time + TIMEOUT))

    if [ "$VERBOSE" = true ]; then
        echo -e "${YELLOW}Checking $name...${NC}"
    fi

    while [ $(date +%s) -lt $end_time ]; do
        if command -v pg_isready &> /dev/null; then
            if pg_isready -h localhost -p "${POSTGRES_PORT:-5432}" -U "${POSTGRES_USER:-app_user}" -d "${POSTGRES_DB:-app_database}" > /dev/null 2>&1; then
                echo -e "${GREEN}✓ $name is healthy${NC}"
                return 0
            fi
        else
            # Fallback: try psql
            if command -v psql &> /dev/null; then
                if PGPASSWORD="${POSTGRES_PASSWORD:-CHANGE_ME}" psql -h localhost -p "${POSTGRES_PORT:-5432}" -U "${POSTGRES_USER:-app_user}" -d "${POSTGRES_DB:-app_database}" -c '\q' > /dev/null 2>&1; then
                    echo -e "${GREEN}✓ $name is healthy${NC}"
                    return 0
                fi
            fi
        fi
        sleep 1
    done

    echo -e "${RED}✗ $name is not responding${NC}"
    return 1
}

# Function to check Redis
check_redis() {
    local name="Redis"
    local start_time=$(date +%s)
    local end_time=$((start_time + TIMEOUT))

    if [ "$VERBOSE" = true ]; then
        echo -e "${YELLOW}Checking $name...${NC}"
    fi

    while [ $(date +%s) -lt $end_time ]; do
        if command -v redis-cli &> /dev/null; then
            if redis-cli -h localhost -p "${REDIS_PORT:-6379}" -a "${REDIS_PASSWORD:-CHANGE_ME}" ping > /dev/null 2>&1; then
                echo -e "${GREEN}✓ $name is healthy${NC}"
                return 0
            fi
        else
            # Fallback: try nc
            if command -v nc &> /dev/null; then
                if echo "PING" | nc -w 1 localhost "${REDIS_PORT:-6379}" | grep -q "PONG"; then
                    echo -e "${GREEN}✓ $name is healthy${NC}"
                    return 0
                fi
            fi
        fi
        sleep 1
    done

    echo -e "${RED}✗ $name is not responding${NC}"
    return 1
}

# Check all services
echo -e "${YELLOW}Checking services (timeout: ${TIMEOUT}s)...${NC}"
echo ""

# HTTP services
for service in "Rust Backend" "Frontend"; do
    url="${SERVICES[$service]}"
    if ! check_http_service "$service" "$url"; then
        FAILED=1
        case "$service" in
            "Rust Backend")
                echo -e "${YELLOW}  Start with: cd backend && cargo run -p api${NC}"
                ;;
            "Frontend")
                echo -e "${YELLOW}  Start with: cd frontend && bun run dev${NC}"
                ;;
        esac
        echo ""
    fi
done

# Database
if ! check_postgres; then
    FAILED=1
    echo -e "${YELLOW}  Start with: docker compose -f compose/dev.yml up -d postgres${NC}"
    echo ""
fi

# Redis
if ! check_redis; then
    FAILED=1
    echo -e "${YELLOW}  Start with: docker compose -f compose/dev.yml up -d redis${NC}"
    echo ""
fi

# Summary
echo -e "${BLUE}========================================${NC}"
if [ $FAILED -eq 0 ]; then
    echo -e "${GREEN}  ✓ All services are healthy${NC}"
else
    echo -e "${RED}  ✗ Some services are down${NC}"
fi
echo -e "${BLUE}========================================${NC}"

exit $FAILED
