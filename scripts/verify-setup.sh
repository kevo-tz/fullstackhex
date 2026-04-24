#!/bin/bash

# Color codes
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}  Verification Script${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""

# Counters
total_checks=0
passed_checks=0

# Check function
check_tool() {
    local cmd=$1
    local display_name=${2:-$cmd}
    
    ((total_checks++))
    
    if command -v "$cmd" &> /dev/null; then
        local version=$("$cmd" --version 2>&1 | head -1)
        echo -e "${GREEN}✓ $display_name installed: $version${NC}"
        ((passed_checks++))
        return 0
    else
        echo -e "${RED}✗ $display_name not installed${NC}"
        return 1
    fi
}

# Check Docker container
check_container() {
    local container_name=$1
    local display_name=${2:-$container_name}
    
    ((total_checks++))
    
    if docker ps --format '{{.Names}}' | grep -q "^$container_name$"; then
        local status=$(docker ps --filter "name=$container_name" --format '{{.Status}}')
        echo -e "${GREEN}✓ $display_name running: $status${NC}"
        ((passed_checks++))
        return 0
    else
        echo -e "${RED}✗ $display_name not running${NC}"
        return 1
    fi
}

# Test port connectivity
test_port() {
    local host=$1
    local port=$2
    local service_name=${3:-"Service"}
    
    ((total_checks++))
    
    if timeout 2 bash -c "echo > /dev/tcp/$host/$port" 2>/dev/null; then
        echo -e "${GREEN}✓ $service_name ($host:$port) is reachable${NC}"
        ((passed_checks++))
        return 0
    else
        echo -e "${RED}✗ $service_name ($host:$port) is not reachable${NC}"
        return 1
    fi
}

# Test Postgres connection
test_postgres() {
    local port=$1
    local db_name=$2
    local display_name=${3:-"Postgres"}
    
    ((total_checks++))
    
    if command -v psql &> /dev/null; then
        if psql -h localhost -p "$port" -U user -d "$db_name" -c "\q" 2>/dev/null; then
            echo -e "${GREEN}✓ $display_name ($port) connection successful${NC}"
            ((passed_checks++))
            return 0
        else
            echo -e "${YELLOW}⚠ $display_name ($port) connection failed (psql available but connection failed)${NC}"
            return 1
        fi
    else
        # Fallback: try TCP connection only
        if test_port localhost "$port" "$display_name"; then
            return 0
        else
            return 1
        fi
    fi
}

echo -e "${YELLOW}Tool Verification:${NC}"
check_tool cargo "Rust (cargo)"
check_tool uv "Python (UV)"
check_tool bun "Bun"
check_tool docker "Docker"
check_tool docker-compose "Docker Compose"

echo ""
echo -e "${YELLOW}Docker Containers:${NC}"
if docker ps -q &>/dev/null; then
    check_container "postgres-rust" "Postgres (Rust)"
    check_container "postgres-python" "Postgres (Python)"
    check_container "minio" "MinIO (RustFS)"
else
    echo -e "${RED}✗ Docker is not running or not accessible${NC}"
fi

echo ""
echo -e "${YELLOW}Service Connectivity:${NC}"
test_postgres 5432 "rust_service" "Postgres - Rust Service"
test_postgres 5433 "python_service" "Postgres - Python Service"
test_port localhost 9000 "MinIO (RustFS)"

echo ""
echo -e "${YELLOW}HTTP Endpoints:${NC}"
if command -v curl &> /dev/null; then
    # Test MinIO endpoint
    if curl -s -m 2 http://localhost:9000/minio/health/live &>/dev/null; then
        echo -e "${GREEN}✓ MinIO health endpoint reachable${NC}"
        ((passed_checks++))
    else
        echo -e "${YELLOW}⚠ MinIO health endpoint not immediately reachable${NC}"
    fi
    ((total_checks++))
fi

echo ""
echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}Verification Summary${NC}"
echo -e "${BLUE}========================================${NC}"
echo -e "Passed: ${GREEN}$passed_checks${NC} / Total: ${BLUE}$total_checks${NC}"

if [ $passed_checks -eq $total_checks ]; then
    echo -e "${GREEN}✓ All checks passed!${NC}"
    echo -e "${BLUE}========================================${NC}"
    exit 0
elif [ $passed_checks -ge $((total_checks - 2)) ]; then
    echo -e "${YELLOW}⚠ Most checks passed (minor issues detected)${NC}"
    echo -e "${BLUE}========================================${NC}"
    exit 0
else
    echo -e "${RED}✗ Some checks failed${NC}"
    echo -e "${BLUE}========================================${NC}"
    exit 1
fi
