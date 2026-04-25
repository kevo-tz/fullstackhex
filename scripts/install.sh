#!/bin/bash
set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}  Bare Metal Demo - Full Initialization${NC}"
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
    rustup update stable
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
    bun upgrade
}

# Check Python (don't auto-install, just check)
check_python() {
    if command -v python3 &> /dev/null; then
        local version=$(python3 --version 2>&1)
        local major=$(python3 -c 'import sys; print(sys.version_info.major)')
        local minor=$(python3 -c 'import sys; print(sys.version_info.minor)')

        if [[ "$major" -ge 3 && "$minor" -ge 14 ]]; then
            echo -e "${GREEN}✓ Python already installed: $version${NC}"
        else
            echo -e "${RED}✗ Python 3.14+ required. Found: $version${NC}"
            echo -e "${YELLOW}  Install with: pyenv install 3.14 or your package manager${NC}"
            return 1
        fi
    else
        echo -e "${RED}✗ Python 3 not found${NC}"
        echo -e "${YELLOW}  Install with: pyenv install 3.14 or your package manager${NC}"
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

# Scaffold Astro frontend
scaffold_frontend() {
    echo ""
    echo -e "${YELLOW}4. Scaffolding Astro frontend...${NC}"

    if [ -d "frontend" ]; then
        echo -e "${GREEN}✓ Frontend already scaffolded${NC}"
        return 0
    fi

    echo "Creating Astro app..."
    bun create astro@latest frontend -- --template minimal --no-install --no-git --yes

    pushd frontend > /dev/null

    echo "Adding Tailwind CSS integration..."
    bunx astro add tailwind --yes

    # Create API health proxy route
    mkdir -p src/pages/api
    cat > src/pages/api/health.ts << 'EOF'
export async function GET() {
    const response = await fetch(`${import.meta.env.VITE_RUST_BACKEND_URL}/health`);
    const body = await response.json();

    return new Response(JSON.stringify(body), {
        headers: { 'Content-Type': 'application/json' },
    });
}
EOF

    echo -e "${GREEN}✓ Astro frontend ready (port 4321)${NC}"

    popd > /dev/null
}

# Create Rust workspace structure
create_rust_workspace() {
    echo ""
    echo -e "${YELLOW}2. Creating Rust workspace...${NC}"
    
    mkdir -p rust-backend
    pushd rust-backend > /dev/null
    
# Create workspace Cargo.toml if not exists
        if [ ! -f Cargo.toml ]; then
            echo "Creating workspace Cargo.toml..."
            cat > Cargo.toml << 'EOF'
[workspace]
members = ["crates/*"]

[workspace.dependencies]
tokio = { version = "1", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
axum = "0.8"
sqlx = { version = "0.8", features = ["postgres", "runtime-tokio-native-tls"] }
tower = "0.5"
tower-http = "0.5"

[profile.release]
lto = true
EOF
    else
        echo -e "${GREEN}✓ Workspace Cargo.toml already exists${NC}"
    fi
    
    # Create crates directory
    mkdir -p crates
    
# Create individual crates if they don't exist
        for crate in api core db python-sidecar; do
            if [ ! -d "crates/$crate" ]; then
                echo "Creating crate: $crate..."
                cargo new --lib --edition 2024 "crates/$crate"
            else
                echo -e "${GREEN}✓ Crate already exists: $crate${NC}"
            fi
        done
    
    # Build workspace
    echo "Building workspace..."
    cargo build --workspace
    echo -e "${GREEN}✓ Rust workspace ready${NC}"
    
    popd > /dev/null
}

# Setup environment
setup_environment() {
    echo ""
    echo -e "${YELLOW}3. Setting up environment...${NC}"
    
    # Copy .env if not exists
    if [ ! -f .env ]; then
        if [ -f .env.example ]; then
            cp .env.example .env
            echo -e "${GREEN}✓ Created .env from .env.example${NC}"
        else
            touch .env
            echo -e "${YELLOW}⚠ .env.example not found, created empty .env${NC}"
        fi
    else
        echo -e "${GREEN}✓ .env already exists${NC}"
    fi
    
    # Add Unix socket path to .env if not present
    if ! grep -q "PYTHON_SIDECAR_SOCKET" .env 2>/dev/null; then
        echo "" >> .env
        echo "# Python Sidecar (Unix socket)" >> .env
        echo "PYTHON_SIDECAR_SOCKET=/tmp/python-sidecar.sock" >> .env
        echo -e "${GREEN}✓ Added Unix socket config to .env${NC}"
    else
        echo -e "${GREEN}✓ Unix socket config already in .env${NC}"
    fi

    if ! grep -q "VITE_RUST_BACKEND_URL" .env 2>/dev/null; then
        echo "" >> .env
        echo "# Frontend → Rust backend" >> .env
        echo "VITE_RUST_BACKEND_URL=http://localhost:8001" >> .env
        echo -e "${GREEN}✓ Added Rust backend URL to .env${NC}"
    else
        echo -e "${GREEN}✓ Rust backend URL already in .env${NC}"
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

# Create workspace, scaffold frontend, and setup environment
create_rust_workspace
setup_environment
scaffold_frontend

echo ""
echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}  ✓ Initialization complete!${NC}"
echo -e "${GREEN}========================================${NC}"
echo ""
echo "Architecture: Rust-centric with Python sidecar (Unix socket)"
echo ""
echo "Next steps:"
echo "  1. docker compose -f docker-compose.dev.yml up -d"
echo "  2. cd rust-backend && cargo run --workspace"
echo "     (Rust will spawn Python sidecar automatically)"
echo "  3. cd frontend && bun run dev"
echo ""
echo "Verify versions:"
echo "  rustc --version    (should show latest stable)"
echo "  bun --version     (should show latest)"
echo "  uv --version      (should show latest)"
echo ""
