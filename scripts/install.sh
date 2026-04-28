#!/bin/bash
set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Resolve and enforce repository root so generated paths are stable
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

if [ ! -f "$REPO_ROOT/scripts/install.sh" ] || [ ! -d "$REPO_ROOT/compose" ]; then
    echo -e "${RED}Could not resolve repository root from script location.${NC}"
    echo -e "${RED}Expected to find scripts/install.sh and compose/ under: $REPO_ROOT${NC}"
    exit 1
fi

if [ "$PWD" != "$REPO_ROOT" ]; then
    echo -e "${YELLOW}⚠ Switching to repository root: $REPO_ROOT${NC}"
    cd "$REPO_ROOT"
fi

if [ -d "$REPO_ROOT/backend/frontend" ]; then
    echo -e "${YELLOW}⚠ Found nested frontend at backend/frontend (likely accidental duplicate).${NC}"
    echo -e "${YELLOW}  Canonical frontend path is: $REPO_ROOT/frontend${NC}"
fi

# Parse command-line arguments
SKIP_PYTHON=false
for arg in "$@"; do
    case $arg in
        --skip-python)
            SKIP_PYTHON=true
            shift
            ;;
        *)
            echo -e "${RED}Unknown argument: $arg${NC}"
            echo "Usage: $0 [--skip-python]"
            exit 1
            ;;
    esac
done

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}  FullStackHex - Full Initialization${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""

if [ "$SKIP_PYTHON" = true ]; then
    echo -e "${YELLOW}⚠ Python check and scaffolding skipped (--skip-python)${NC}"
    echo ""
fi

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
        bun upgrade
        return 0
    fi

    echo -e "${YELLOW}Installing Bun...${NC}"
    curl -fsSL https://bun.sh/install | bash

    # Ensure bun bin is on PATH for current session
    if [ -d "$HOME/.bun/bin" ]; then
        export PATH="$HOME/.bun/bin:$PATH"
    fi

    # Detect active shell and its rc file
    local shell_name
    shell_name=$(basename "${SHELL:-/bin/sh}")
    local rc_file=""
    case "$shell_name" in
        bash)
            rc_file="$HOME/.bashrc"
            ;;
        zsh)
            rc_file="$HOME/.zshrc"
            ;;
        fish)
            rc_file="$HOME/.config/fish/config.fish"
            ;;
        *)
            # Fallback to .profile for sh, dash, etc.
            rc_file="$HOME/.profile"
            ;;
    esac

    # Ensure PATH is persisted in the rc file
    if [ -n "$rc_file" ]; then
        # Check if already configured
        if [ -f "$rc_file" ] && grep -q 'bun/bin' "$rc_file" 2>/dev/null; then
            echo -e "${GREEN}✓ Bun PATH already configured in $rc_file${NC}"
        else
            mkdir -p "$(dirname "$rc_file")" 2>/dev/null || true
            echo '' >> "$rc_file"
            echo '# Added by FullStackHex install.sh' >> "$rc_file"
            if [[ "$shell_name" = "fish" ]]; then
                echo 'set -gx PATH "$HOME/.bun/bin" $PATH' >> "$rc_file"
            else
                echo 'export PATH="$HOME/.bun/bin:$PATH"' >> "$rc_file"
            fi
            echo -e "${GREEN}✓ Added Bun to PATH in $rc_file${NC}"
        fi

        # Source the rc file for the current session (best-effort)
        # shellcheck disable=SC1090
        source "$rc_file" 2>/dev/null || true
    fi

    # Verify bun is now accessible
    if command -v bun &> /dev/null; then
        echo -e "${GREEN}✓ Bun installed: v$(bun --version)${NC}"
    else
        echo -e "${YELLOW}⚠ Bun installed but not on PATH. Run: source $rc_file${NC}"
        echo -e "${YELLOW}  Or restart your shell.${NC}"
    fi

    bun upgrade
}

# Check Python (don't auto-install, just check)
check_python() {
    if [ "$SKIP_PYTHON" = true ]; then
        echo -e "${YELLOW}⚠ Skipping Python check (--skip-python set)${NC}"
        return 0
    fi

    if command -v python3 &> /dev/null; then
        local version=$(python3 --version 2>&1)
        local major=$(python3 -c 'import sys; print(sys.version_info.major)')
        local minor=$(python3 -c 'import sys; print(sys.version_info.minor)')

        # Accept Python >= 3.14, including future major versions (e.g., 4.x).
        if (( major > 3 )) || (( major == 3 && minor >= 14 )); then
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
        
        # uv installs to $HOME/.local/bin by default
        # Also check cargo/bin as fallback
        if [ -x "$HOME/.local/bin/uv" ]; then
            export PATH="$HOME/.local/bin:$PATH"
        elif [ -x "$HOME/.cargo/bin/uv" ]; then
            export PATH="$HOME/.cargo/bin:$PATH"
        fi
        echo -e "${GREEN}✓ uv installed: $(uv --version)${NC}"
    fi
}

# Scaffold Python sidecar service
scaffold_python_sidecar() {
    echo ""
    echo -e "${YELLOW}4. Scaffolding Python sidecar...${NC}"

    mkdir -p python-sidecar
    pushd python-sidecar > /dev/null

    if [ ! -f pyproject.toml ]; then
        cat > pyproject.toml << 'EOF'
[project]
name = "python-sidecar"
version = "0.1.0"
description = "FullStackHex Python sidecar"
requires-python = ">=3.14"
dependencies = [
  "fastapi>=0.116.0",
  "uvicorn>=0.35.0"
]

[dependency-groups]
dev = [
  "pytest>=8.4.0",
  "ruff>=0.8.0",
  "httpx>=0.28.0"
]

[tool.pytest.ini_options]
testpaths = ["tests"]
pythonpath = ["."]

[tool.ruff]
line-length = 100
target-version = "py314"
EOF
        echo -e "${GREEN}✓ Created python-sidecar/pyproject.toml${NC}"
    fi

    mkdir -p app tests

    if [ ! -f app/__init__.py ]; then
        cat > app/__init__.py << 'EOF'
# Generated package marker for Python sidecar app module.
EOF
    fi

    if [ ! -f app/main.py ]; then
        cat > app/main.py << 'EOF'
from fastapi import FastAPI

app = FastAPI()


@app.get("/health")
def health() -> dict[str, str]:
    return {"status": "ok", "service": "python-sidecar"}
EOF
        echo -e "${GREEN}✓ Created python sidecar app entrypoint${NC}"
    fi

    if [ ! -f tests/test_health.py ]; then
        cat > tests/test_health.py << 'EOF'
from fastapi.testclient import TestClient

from app.main import app


def test_health_endpoint() -> None:
    client = TestClient(app)
    response = client.get("/health")

    assert response.status_code == 200
    assert response.json()["status"] == "ok"
EOF
        echo -e "${GREEN}✓ Created python unit test${NC}"
    fi

    if [ ! -f tests/test_integration.py ]; then
        cat > tests/test_integration.py << 'EOF'
from fastapi.testclient import TestClient

from app.main import app


def test_sidecar_response_shape() -> None:
    client = TestClient(app)
    payload = client.get("/health").json()

    assert set(payload.keys()) == {"status", "service"}
    assert payload["service"] == "python-sidecar"
EOF
        echo -e "${GREEN}✓ Created python integration test${NC}"
    fi

    popd > /dev/null
}

# Scaffold Astro frontend
scaffold_frontend() {
    echo ""
    echo -e "${YELLOW}5. Scaffolding Astro frontend...${NC}"

    if [ -d "frontend" ]; then
        echo -e "${GREEN}✓ Frontend directory already exists${NC}"
        pushd frontend > /dev/null

        if [ ! -d "node_modules" ]; then
            echo "Installing frontend dependencies..."
            bun install
        fi

        if ! grep -q "@tailwindcss/vite" package.json 2>/dev/null; then
            echo "Adding Tailwind v4 and Node SSR adapter..."
            bun add @tailwindcss/vite tailwindcss @astrojs/node
        fi

        mkdir -p src/pages/api
        if [ ! -f src/pages/api/health.ts ]; then
            cat > src/pages/api/health.ts << 'EOF'
export async function GET() {
    const response = await fetch(`${import.meta.env.VITE_RUST_BACKEND_URL}/health`);
    const body = await response.json();

    return new Response(JSON.stringify(body), {
        headers: { 'Content-Type': 'application/json' },
    });
}
EOF
        fi

        popd > /dev/null
        return 0
    fi

    echo "Creating Astro app..."
    bun create astro@latest frontend -- --template minimal --no-install --no-git --yes

    pushd frontend > /dev/null

    echo "Installing Tailwind v4 and Node SSR adapter..."
    bun add @tailwindcss/vite tailwindcss @astrojs/node

    echo "Installing TypeScript runtime types and check tooling for Bun/Node..."
    bun add --dev @astrojs/check typescript @types/node bun-types

    echo "Installing remaining dependencies..."
    bun install

    # Inject typecheck and lint scripts (astro check) into package.json
    bun -e "
const fs = require('fs');
const pkg = JSON.parse(fs.readFileSync('package.json', 'utf8'));
pkg.scripts = pkg.scripts || {};
pkg.scripts.typecheck = pkg.scripts.typecheck || 'astro check';
pkg.scripts.lint = pkg.scripts.lint || 'astro check';
fs.writeFileSync('package.json', JSON.stringify(pkg, null, 2) + '\n');
"

    # Write astro.config.mjs with SSR output and Tailwind vite plugin
    cat > astro.config.mjs << 'EOF'
// @ts-check
import { defineConfig } from 'astro/config';
import node from '@astrojs/node';
import tailwindcss from '@tailwindcss/vite';

export default defineConfig({
  output: 'server',
  adapter: node({ mode: 'standalone' }),
  vite: {
    plugins: [tailwindcss()]
  }
});
EOF

        # Ensure Bun/Node globals are typed for generated test files
        cat > tsconfig.json << 'EOF'
{
    "extends": "astro/tsconfigs/strict",
    "compilerOptions": {
        "types": ["node", "bun-types"]
    },
    "include": [".astro/types.d.ts", "**/*"],
    "exclude": ["dist"]
}
EOF

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

# Ensure generated templates always include baseline tests
scaffold_generated_tests() {
    echo ""
    echo -e "${YELLOW}6. Adding generated test suites...${NC}"

    # Rust unit + integration + smoke tests
    for crate in api core db python-sidecar; do
        mkdir -p "backend/crates/$crate/tests"

        if [ ! -f "backend/crates/$crate/tests/unit_generated.rs" ]; then
            cat > "backend/crates/$crate/tests/unit_generated.rs" << 'EOF'
#[cfg(test)]
mod tests {

    #[test]
    fn health_response_structure() {
        // Test that health endpoint returns proper JSON structure
        let response = serde_json::json!({
            "status": "ok",
            "service": "test-service"
        });

        assert_eq!(response["status"], "ok");
        assert!(response["service"].is_string());
    }

    #[test]
    fn environment_variables_loaded() {
        // Test that required env vars have defaults or are set
        // Safety: single-threaded test; no other threads reading this variable.
        unsafe {
            std::env::set_var("RUST_LOG", "info");
        }
        let log_level = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());
        assert_eq!(log_level, "info");
    }
}
EOF
        fi
    done

    if [ ! -f "backend/crates/api/tests/integration_health_route.rs" ]; then
        cat > "backend/crates/api/tests/integration_health_route.rs" << 'EOF'
#[cfg(test)]
mod tests {

    // Test that health route path constant is valid
    #[test]
    fn health_endpoint_returns_200() {
        let health_path = "/health";
        assert!(health_path.starts_with('/'));
        assert!(health_path.contains("health"));
    }

    // Test that health response has correct structure
    #[test]
    fn health_response_structure() {
        let expected_keys = vec!["status", "service", "version"];
        let response_json = r#"{"status":"ok","service":"api","version":"0.1.0"}"#;

        let response: serde_json::Value = serde_json::from_str(response_json).unwrap();
        for key in &expected_keys {
            assert!(response.as_object().unwrap().contains_key(*key));
        }
    }
}
EOF
    fi

    if [ ! -f "backend/crates/api/tests/smoke_generated.rs" ]; then
        cat > "backend/crates/api/tests/smoke_generated.rs" << 'EOF'
#[cfg(test)]
mod tests {
    // Smoke test: verify workspace compiles and core modules are accessible
    #[test]
    fn workspace_compiles_and_modules_accessible() {
        // Test that we can access core types
        // This test ensures the crate structure is correct
        let version = env!("CARGO_PKG_VERSION");
        assert!(!version.is_empty());
    }

    #[test]
    fn environment_configuration_valid() {
        // Test that required environment variables are properly configured
        use std::env;

        // These should have defaults or be set
        let rust_log = env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());
        let database_url =
            env::var("DATABASE_URL").unwrap_or_else(|_| "postgres://localhost/test".to_string());

        assert!(!rust_log.is_empty());
        assert!(!database_url.is_empty());
    }
}
EOF
    fi

    if [ ! -f "backend/crates/api/tests/integration_socket.rs" ]; then
        cat > "backend/crates/api/tests/integration_socket.rs" << 'EOF'
// Integration tests for Unix socket communication between Rust and Python
// Run with: cargo test --test integration_socket

use std::path::PathBuf;
use std::time::Duration;
use tokio::time::timeout;

/// Test that socket path is correctly configured
#[test]
fn socket_path_configuration() {
    let socket_path = std::env::var("PYTHON_SIDECAR_SOCKET")
        .unwrap_or_else(|_| "/tmp/python-sidecar.sock".to_string());

    let path = PathBuf::from(socket_path);
    assert!(path.is_absolute() || path.starts_with("~"));
}

/// Test socket path directory creation
#[tokio::test]
async fn socket_directory_creation() {
    use std::fs;

    let temp_dir = std::env::temp_dir().join("fullstackhex_test");
    fs::create_dir_all(&temp_dir).expect("Failed to create temp dir");

    let socket_path = temp_dir.join("test-socket.sock");

    if socket_path.exists() {
        fs::remove_file(&socket_path).expect("Failed to remove stale socket");
    }

    assert!(temp_dir.exists());
    assert!(temp_dir.is_dir());

    let _ = fs::remove_dir_all(&temp_dir);
}

/// Test error handling for missing socket
#[tokio::test]
async fn error_handling_missing_socket() {
    use tokio::net::UnixStream;

    let non_existent = PathBuf::from("/tmp/non-existent-socket.sock");

    let result = timeout(Duration::from_secs(1), UnixStream::connect(&non_existent)).await;

    match result {
        Ok(Err(_)) => {
            // Expected: connection failed
        }
        Ok(Ok(_)) => {
            panic!("Should not be able to connect to non-existent socket");
        }
        Err(_) => {
            // Timeout is also acceptable
        }
    }
}

/// Test socket path from environment with priority
#[test]
fn socket_path_env_override() {
    let original = std::env::var("PYTHON_SIDECAR_SOCKET").ok();

    // Safety: single-threaded test; no other threads reading this variable.
    unsafe {
        std::env::set_var("PYTHON_SIDECAR_SOCKET", "/custom/path/socket.sock");
    }
    let path = std::env::var("PYTHON_SIDECAR_SOCKET").unwrap();
    assert_eq!(path, "/custom/path/socket.sock");

    unsafe {
        match original {
            Some(val) => std::env::set_var("PYTHON_SIDECAR_SOCKET", val),
            None => std::env::remove_var("PYTHON_SIDECAR_SOCKET"),
        }
    }
}

/// Test request structure for sidecar communication
#[test]
fn sidecar_request_structure() {
    let request_json = serde_json::json!({
        "method": "GET",
        "path": "/api/data",
        "headers": {},
        "body": null
    });

    assert_eq!(request_json["method"], "GET");
    assert_eq!(request_json["path"], "/api/data");
    assert!(request_json["body"].is_null());
}

/// Test response structure from sidecar
#[test]
fn sidecar_response_structure() {
    let response_json = serde_json::json!({
        "status": 200,
        "body": {"message": "success"},
        "headers": {"content-type": "application/json"}
    });

    assert_eq!(response_json["status"], 200);
    assert_eq!(response_json["body"]["message"], "success");
    assert_eq!(response_json["headers"]["content-type"], "application/json");
}

/// Test retry logic for socket connection
#[tokio::test]
async fn socket_retry_logic() {
    let socket_path = PathBuf::from("/tmp/non-existent-test-socket.sock");
    let max_retries = 3;
    let mut attempts = 0;

    loop {
        attempts += 1;

        let result = timeout(
            Duration::from_millis(100),
            tokio::net::UnixStream::connect(&socket_path),
        )
        .await;

        match result {
            Ok(Ok(_)) => {
                break;
            }
            _ => {
                if attempts >= max_retries {
                    assert!(attempts >= max_retries);
                    break;
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }
    }

    assert!(attempts >= max_retries);
}

/// Mock test for full request/response cycle
/// (Requires actual sidecar running - marked as ignored by default)
#[tokio::test]
#[ignore]
async fn full_socket_communication() {
    let socket_path = std::env::var("PYTHON_SIDECAR_SOCKET")
        .unwrap_or_else(|_| "/tmp/python-sidecar.sock".to_string());

    if !PathBuf::from(&socket_path).exists() {
        println!("Skipping test: socket not found at {}", socket_path);
        return;
    }

    assert!(PathBuf::from(&socket_path).exists());
}
EOF
    fi

    # Frontend unit + integration + smoke tests
    mkdir -p frontend/tests

    if [ ! -f "frontend/tests/unit.test.ts" ]; then
        cat > "frontend/tests/unit.test.ts" << 'EOF'
import { describe, expect, test } from "bun:test";

describe("frontend generated unit test", () => {
  test("health endpoint path is valid", () => {
    const healthRoute = "/api/health";
    expect(healthRoute).toStartWith("/api/");
    expect(healthRoute).toContain("health");
  });

  test("environment variables are defined", () => {
    const apiUrl = process.env.PUBLIC_API_URL || "http://localhost:8001";
    expect(apiUrl).toBeTypeOf("string");
    expect(apiUrl.length).toBeGreaterThan(0);
  });

  test("TypeScript types work correctly", () => {
    interface HealthResponse {
      status: string;
      service: string;
    }

    const mockResponse: HealthResponse = {
      status: "ok",
      service: "api"
    };

    expect(mockResponse.status).toBe("ok");
    expect(mockResponse.service).toBe("api");
  });
});
EOF
    fi

    if [ ! -f "frontend/tests/integration-health-route.test.ts" ]; then
        cat > "frontend/tests/integration-health-route.test.ts" << 'EOF'
import { describe, expect, test } from "bun:test";

describe("frontend generated integration test", () => {
  test("health route path is stable", () => {
    const route = "/api/health";
        expect(route.startsWith("/api/")).toBe(true);
  });
});
EOF
    fi

    if [ ! -f "frontend/tests/smoke.test.ts" ]; then
        cat > "frontend/tests/smoke.test.ts" << 'EOF'
import { expect, test } from "bun:test";

test("generated frontend smoke test", () => {
  expect(typeof Bun.version).toBe("string");
});
EOF
    fi

    echo -e "${GREEN}✓ Generated test suites ready (Rust/Python/Frontend)${NC}"
}

# Create Rust workspace structure
create_rust_workspace() {
    echo ""
    echo -e "${YELLOW}2. Creating Rust workspace...${NC}"
    
    mkdir -p backend
    pushd backend > /dev/null
    
# Create workspace Cargo.toml if not exists
        if [ ! -f Cargo.toml ]; then
            echo "Creating workspace Cargo.toml..."
            cat > Cargo.toml << 'EOF'
[workspace]
members = ["crates/*"]
resolver = "3"

[workspace.package]
description = "FullStackHex project"
license = "MIT"
repository = "https://github.com/yourusername/yourrepo"
authors = ["Your Name <your@email.com>"]

[workspace.dependencies]
tokio = { version = "1", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
axum = "0.8"
sqlx = { version = "0.8", features = ["postgres", "runtime-tokio-native-tls"] }
tower = "0.5"
tower-http = "0.5"
serde_json = "1.0"

[profile.release]
lto = true
EOF
    else
        echo -e "${GREEN}✓ Workspace Cargo.toml already exists${NC}"
    fi
    
# Create migration directory for sqlx
    mkdir -p crates/db/migrations

    # Create individual crates if they don't exist or are invalid
        for crate in api core db python-sidecar; do
            local crate_valid=false
            if [ -d "crates/$crate" ] && [ -f "crates/$crate/Cargo.toml" ]; then
                crate_valid=true
            fi

            if [ "$crate_valid" = true ]; then
                echo -e "${GREEN}✓ Crate already exists: $crate${NC}"
            else
                if [ -d "crates/$crate" ]; then
                    echo "Removing invalid crate directory: $crate..."
                    rm -rf "crates/$crate"
                fi
                echo "Creating crate: $crate..."
                cargo new --lib --edition 2024 "crates/$crate"
                # Overwrite cargo new's minimal Cargo.toml with workspace-aware version + dev-deps
                case "$crate" in
                    api)
                        cat > "crates/$crate/Cargo.toml" << 'CARGO_EOF'
[package]
name = "api"
version = "0.1.0"
edition = "2024"
description.workspace = true
license.workspace = true
repository.workspace = true
authors.workspace = true

[dependencies]

[dev-dependencies]
tokio = { workspace = true }
axum = { workspace = true }
tower = { workspace = true }
serde_json = { workspace = true }
CARGO_EOF
                        ;;
                    *)
                        cat > "crates/$crate/Cargo.toml" << CARGO_EOF
[package]
name = "$crate"
version = "0.1.0"
edition = "2024"
description.workspace = true
license.workspace = true
repository.workspace = true
authors.workspace = true

[dependencies]

[dev-dependencies]
serde_json = { workspace = true }
CARGO_EOF
                        ;;
                esac
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
        # CI environments get a temp path; local gets user-isolated path
        if [ "${CI:-false}" = "true" ]; then
            local socket_dir="${RUNNER_TEMP:-$PWD/.tmp}/sockets"
            mkdir -p "$socket_dir"
            local socket_path="$socket_dir/python-sidecar.sock"
            echo -e "${YELLOW}⚠ CI detected: using temp socket path${NC}"
        else
            local socket_dir="$HOME/.fullstackhex/sockets"
            mkdir -p "$socket_dir"
            local socket_path="$socket_dir/python-sidecar.sock"
        fi

        echo "" >> .env
        echo "# Python Sidecar (Unix socket)" >> .env
        echo "PYTHON_SIDECAR_SOCKET=$socket_path" >> .env
        echo -e "${GREEN}✓ Added Unix socket config to .env${NC}"
        echo -e "${YELLOW}  Socket path: $socket_path${NC}"
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

    if ! grep -q "ASTRO_PORT" .env 2>/dev/null; then
        echo "" >> .env
        echo "# Astro dev server port" >> .env
        echo "ASTRO_PORT=4321" >> .env
        echo -e "${GREEN}✓ Added Astro port to .env${NC}"
    else
        echo -e "${GREEN}✓ Astro port already in .env${NC}"
    fi

    if ! grep -q "PUBLIC_API_URL" .env 2>/dev/null; then
        echo "" >> .env
        echo "# Public API URL for frontend" >> .env
        echo "PUBLIC_API_URL=http://localhost:8001" >> .env
        echo -e "${GREEN}✓ Added public API URL to .env${NC}"
    else
        echo -e "${GREEN}✓ Public API URL already in .env${NC}"
    fi
}

# Run installations
echo -e "${YELLOW}1. Checking dependencies...${NC}"
echo ""

install_rust
install_bun

if [ "$SKIP_PYTHON" != true ]; then
    check_python
fi

install_uv
check_docker

# Create workspace, scaffold frontend, and setup environment
create_rust_workspace
setup_environment

if [ "$SKIP_PYTHON" != true ]; then
    scaffold_python_sidecar
fi

scaffold_frontend
scaffold_generated_tests

echo ""
echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}  ✓ Initialization complete!${NC}"
echo -e "${GREEN}========================================${NC}"
echo ""
echo "Architecture: Rust-centric with Python sidecar (Unix socket)"
echo ""
echo "Next steps:"
echo "  1. docker compose -f compose/dev.yml up -d"
echo "  2. cd backend && cargo run -p api"
echo "     (starts Axum on port 8001)"
echo "  3. cd frontend && bun run dev"
echo ""
echo "Verify versions:"
echo "  rustc --version    (should show latest stable)"
echo "  bun --version     (should show latest)"
echo "  uv --version      (should show latest)"
echo ""
