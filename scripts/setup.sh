#!/bin/bash

# Color codes
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Get script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}  Bare Metal Demo - Setup Script${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""

# Error handler
error_exit() {
    echo -e "${RED}✗ Error: $1${NC}"
    exit 1
}

# Check command exists
check_command() {
    local cmd=$1
    local display_name=${2:-$cmd}
    
    if command -v "$cmd" &> /dev/null; then
        local version=$("$cmd" --version 2>&1 | head -1)
        echo -e "${GREEN}✓ $display_name: $version${NC}"
        return 0
    else
        echo -e "${RED}✗ $display_name not found${NC}"
        return 1
    fi
}

# Check all required tools
echo -e "${YELLOW}1. Checking required tools...${NC}"
local_failed=0

check_command cargo "Rust (cargo)" || local_failed=1
check_command uv "Python (UV)" || local_failed=1
check_command bun "Bun" || local_failed=1
check_command docker "Docker" || local_failed=1
check_command docker-compose "Docker Compose" || local_failed=1

if [ $local_failed -eq 1 ]; then
    error_exit "Some required tools are missing. Please install them and try again."
fi

echo ""

# Create .env if it doesn't exist
echo -e "${YELLOW}2. Creating .env file...${NC}"
if [ ! -f "$PROJECT_ROOT/.env" ]; then
    if [ -f "$PROJECT_ROOT/.env.example" ]; then
        cp "$PROJECT_ROOT/.env.example" "$PROJECT_ROOT/.env"
        echo -e "${GREEN}✓ Created .env from .env.example${NC}"
    else
        error_exit ".env.example not found at $PROJECT_ROOT/.env.example"
    fi
else
    echo -e "${GREEN}✓ .env already exists${NC}"
fi

echo ""

# Load environment variables
set -a
source "$PROJECT_ROOT/.env"
set +a

# Docker Compose operations
echo -e "${YELLOW}3. Pulling Docker images...${NC}"
cd "$PROJECT_ROOT" || error_exit "Cannot change to project root: $PROJECT_ROOT"

if ! docker-compose pull; then
    error_exit "Failed to pull Docker images"
fi
echo -e "${GREEN}✓ Docker images pulled successfully${NC}"

echo ""

echo -e "${YELLOW}4. Starting Docker containers...${NC}"
if ! docker-compose up -d --profile manual; then
    error_exit "Failed to start Docker containers"
fi
echo -e "${GREEN}✓ Docker containers started${NC}"

echo ""

# Wait for services to be ready
echo -e "${YELLOW}5. Waiting for services to be ready...${NC}"
sleep 3

# Initialize Rust backend if needed
echo -e "${YELLOW}6. Initializing services...${NC}"

if [ -d "$PROJECT_ROOT/rust-backend" ]; then
    echo "  - Checking Rust backend..."
    cd "$PROJECT_ROOT/rust-backend" || error_exit "Cannot change to rust-backend directory"
    
    if [ -f "Cargo.toml" ]; then
        echo "    Building Rust backend..."
        if cargo build --release 2>&1 | grep -q "Finished"; then
            echo -e "${GREEN}    ✓ Rust backend built${NC}"
        else
            echo -e "${YELLOW}    ⚠ Rust backend build check completed${NC}"
        fi
    fi
fi

if [ -d "$PROJECT_ROOT/python-services" ]; then
    echo "  - Checking Python services..."
    cd "$PROJECT_ROOT/python-services" || error_exit "Cannot change to python-services directory"
    
    if [ -f "pyproject.toml" ]; then
        echo "    Installing Python dependencies..."
        if uv sync 2>&1 | tail -1 | grep -q "Synced"; then
            echo -e "${GREEN}    ✓ Python dependencies installed${NC}"
        else
            echo -e "${YELLOW}    ⚠ Python dependencies sync attempted${NC}"
        fi
    fi
fi

if [ -d "$PROJECT_ROOT/typescript-frontend" ]; then
    echo "  - Checking TypeScript frontend..."
    cd "$PROJECT_ROOT/typescript-frontend" || error_exit "Cannot change to typescript-frontend directory"
    
    if [ -f "package.json" ]; then
        echo "    Installing Node dependencies..."
        if bun install 2>&1 | tail -1 | grep -q "packages installed"; then
            echo -e "${GREEN}    ✓ Node dependencies installed${NC}"
        else
            echo -e "${YELLOW}    ⚠ Node dependencies install attempted${NC}"
        fi
    fi
fi

echo ""

# Run verification script
echo -e "${YELLOW}7. Running verification...${NC}"
if [ -f "$SCRIPT_DIR/verify-setup.sh" ]; then
    bash "$SCRIPT_DIR/verify-setup.sh"
    verify_result=$?
    
    if [ $verify_result -eq 0 ]; then
        echo ""
        echo -e "${GREEN}========================================${NC}"
        echo -e "${GREEN}  ✓ Setup completed successfully!${NC}"
        echo -e "${GREEN}========================================${NC}"
        exit 0
    else
        echo ""
        echo -e "${YELLOW}========================================${NC}"
        echo -e "${YELLOW}  ⚠ Setup completed with issues${NC}"
        echo -e "${YELLOW}  Please check the verification output${NC}"
        echo -e "${YELLOW}========================================${NC}"
        exit 1
    fi
else
    error_exit "verify-setup.sh not found at $SCRIPT_DIR/verify-setup.sh"
fi
