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
echo -e "${BLUE}  Cleanup Script${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""

# Error handler
error_exit() {
    echo -e "${RED}✗ Error: $1${NC}"
    exit 1
}

# Confirm cleanup
echo -e "${YELLOW}This will:${NC}"
echo "  - Stop and remove Docker containers"
echo "  - Remove Docker volumes"
echo "  - Delete node_modules directories"
echo "  - Delete target/ directories (Rust builds)"
echo "  - Delete __pycache__ directories (Python cache)"
echo "  - Delete dist/ directories (built artifacts)"
echo ""
read -p "Are you sure you want to continue? (yes/no): " -r confirm

if [[ ! $confirm =~ ^[Yy][Ee][Ss]$ ]]; then
    echo -e "${YELLOW}Cleanup cancelled${NC}"
    exit 0
fi

echo ""
echo -e "${YELLOW}Starting cleanup...${NC}"
echo ""

# Change to project root
cd "$PROJECT_ROOT" || error_exit "Cannot change to project root: $PROJECT_ROOT"

# Stop and remove Docker containers
echo -e "${YELLOW}1. Stopping Docker containers...${NC}"
if command -v docker-compose &> /dev/null; then
    if docker-compose down --volumes 2>&1 | grep -q "Removing"; then
        echo -e "${GREEN}✓ Docker containers and volumes removed${NC}"
    else
        echo -e "${YELLOW}⚠ Docker containers already stopped or not running${NC}"
    fi
else
    echo -e "${YELLOW}⚠ docker-compose not found, skipping container cleanup${NC}"
fi

echo ""

# Remove build artifacts
echo -e "${YELLOW}2. Removing build artifacts...${NC}"

# Remove node_modules
if [ -d "typescript-frontend/node_modules" ]; then
    echo "  Removing typescript-frontend/node_modules..."
    rm -rf typescript-frontend/node_modules
    echo -e "${GREEN}  ✓ Removed${NC}"
fi

# Remove Rust target directories
if [ -d "rust-backend/target" ]; then
    echo "  Removing rust-backend/target..."
    rm -rf rust-backend/target
    echo -e "${GREEN}  ✓ Removed${NC}"
fi

# Remove Python cache and venv
if [ -d "python-services/__pycache__" ]; then
    echo "  Removing python-services/__pycache__..."
    rm -rf python-services/__pycache__
    echo -e "${GREEN}  ✓ Removed${NC}"
fi

if [ -d "python-services/.venv" ]; then
    echo "  Removing python-services/.venv..."
    rm -rf python-services/.venv
    echo -e "${GREEN}  ✓ Removed${NC}"
fi

# Remove dist directories
find . -type d -name "dist" -not -path "./.git/*" 2>/dev/null | while read -r dir; do
    echo "  Removing $dir..."
    rm -rf "$dir"
    echo -e "${GREEN}  ✓ Removed${NC}"
done

# Remove build directories
find . -type d -name "build" -not -path "./.git/*" 2>/dev/null | while read -r dir; do
    echo "  Removing $dir..."
    rm -rf "$dir"
    echo -e "${GREEN}  ✓ Removed${NC}"
done

echo ""
echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}  ✓ Cleanup completed successfully!${NC}"
echo -e "${GREEN}========================================${NC}"
echo ""
echo -e "${YELLOW}Next steps:${NC}"
echo "  - Run './scripts/setup.sh' to reinitialize the project"
echo ""
