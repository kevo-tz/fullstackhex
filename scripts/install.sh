#!/bin/bash
set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}  Bare Metal Template - Install Script${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""

# Detect OS
detect_os() {
    if [[ "$OSTYPE" == "linux-gnu"* ]]; then
        echo "linux"
    elif [[ "$OSTYPE" == "darwin"* ]]; then
        echo "macos"
    else
        echo "unknown"
    fi
}

OS=$(detect_os)
echo -e "${YELLOW}Detected OS: $OS${NC}"
echo ""

# Check and install Rust
install_rust() {
    if command -v rustc &> /dev/null; then
        local version=$(rustc --version)
        echo -e "${GREEN}✓ Rust already installed: $version${NC}"
    else
        echo -e "${YELLOW}Installing Rust...${NC}"
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        source "$HOME/.cargo/env"
        echo -e "${GREEN}✓ Rust installed: $(rustc --version)${NC}"
    fi
}

# Check and install Bun
install_bun() {
    if command -v bun &> /dev/null; then
        local version=$(bun --version)
        echo -e "${GREEN}✓ Bun already installed: v$version${NC}"
    else
        echo -e "${YELLOW}Installing Bun...${NC}"
        curl -fsSL https://bun.sh/install | bash
        export PATH="$HOME/.bun/bin:$PATH"
        echo -e "${GREEN}✓ Bun installed: v$(bun --version)${NC}"
    fi
}

# Check Python (don't auto-install, just check)
check_python() {
    if command -v python3 &> /dev/null; then
        local version=$(python3 --version 2>&1)
        local major=$(python3 -c 'import sys; print(sys.version_info.major)')
        local minor=$(python3 -c 'import sys; print(sys.version_info.minor)')

        if [[ "$major" -ge 3 && "$minor" -ge 11 ]]; then
            echo -e "${GREEN}✓ Python already installed: $version${NC}"
        else
            echo -e "${RED}✗ Python 3.11+ required. Found: $version${NC}"
            echo -e "${YELLOW}  Install with: pyenv install 3.11 or your package manager${NC}"
            return 1
        fi
    else
        echo -e "${RED}✗ Python 3 not found${NC}"
        echo -e "${YELLOW}  Install with: pyenv install 3.11 or your package manager${NC}"
        return 1
    fi
}

# Check Docker (don't auto-install)
check_docker() {
    if command -v docker &> /dev/null; then
        local version=$(docker --version)
        echo -e "${GREEN}✓ Docker already installed: $version${NC}"
    else
        echo -e "${RED}✗ Docker not found - please install manually${NC}"
        echo -e "${YELLOW}  Visit: https://docs.docker.com/get-docker/${NC}"
        return 1
    fi

    if command -v docker-compose &> /dev/null || docker compose version &> /dev/null; then
        echo -e "${GREEN}✓ Docker Compose available${NC}"
    else
        echo -e "${RED}✗ Docker Compose not found${NC}"
        return 1
    fi
}

# Check and install uv (Python package manager)
install_uv() {
    if command -v uv &> /dev/null; then
        local version=$(uv --version)
        echo -e "${GREEN}✓ uv already installed: $version${NC}"
    else
        echo -e "${YELLOW}Installing uv (Python package manager)...${NC}"
        curl -LsSf https://astral.sh/uv/install.sh | sh
        export PATH="$HOME/.cargo/bin:$PATH"
        echo -e "${GREEN}✓ uv installed: $(uv --version)${NC}"
    fi
}

# Run installations
echo -e "${YELLOW}1. Checking dependencies...${NC}"
echo ""

install_rust
install_bun
check_python
install_uv
check_docker

echo ""
echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}  ✓ Installation check complete!${NC}"
echo -e "${GREEN}========================================${NC}"
echo ""
echo "Next steps:"
echo "  - Dev:    docker compose -f docker-compose.dev.yml up -d"
echo "  - Prod:   docker compose -f docker-compose.prod.yml up -d"
echo "  - Rust:   cd rust-backend && cargo run"
echo "  - Python: cd python-services && uv run uvicorn src.main:app --reload"
echo "  - Frontend: cd frontend && bun run dev"
echo ""
