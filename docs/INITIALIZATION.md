# Project Initialization Template

Use this as a template for new projects with the same architecture.

## Table of Contents

1. [Prerequisites](#prerequisites)
2. [Initialization Script](#initialization-script)
3. [Scaffold Astro Frontend](#scaffold-astro-frontend)
4. [Make Script Executable](#make-script-executable)
5. [Verification](#verification)
6. [Portable Template](#portable-template)
7. [What Gets Installed (Latest Versions)](#what-gets-installed-latest-versions)
8. [Troubleshooting](#troubleshooting)
9. [Related Docs](#related-docs)

## Prerequisites

- Linux/macOS (Unix domain socket requires Unix-like OS)
- Git
- Internet connection

## Initialization Script

> **Note:** This is a portable template showing the core steps. The project's `scripts/install.sh` extends this with OS detection, color output, and additional safety checks.

Save as `scripts/install.sh`:

```bash
#!/bin/bash
set -e

echo "=== FullStackHex Initialization ==="

# 1. Check Python 3.14+ (required for sidecar; install manually via pyenv if missing)
if command -v python3 &> /dev/null; then
    major=$(python3 -c 'import sys; print(sys.version_info.major)')
    minor=$(python3 -c 'import sys; print(sys.version_info.minor)')
    if (( major < 3 )) || (( major == 3 && minor < 14 )); then
        echo "ERROR: Python 3.14+ required. Found: $(python3 --version)"
        echo "Install with: pyenv install 3.14"
        exit 1
    fi
else
    echo "ERROR: Python 3 not found. Install Python 3.14+ first."
    exit 1
fi

# 2. Install Rust (edition 2024)
if ! command -v rustc &> /dev/null; then
    echo "Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
fi
rustup update stable
rustc --version

# 3. Install Bun
if ! command -v bun &> /dev/null; then
    echo "Installing Bun..."
    curl -fsSL https://bun.sh/install | bash
    source "$HOME/.bashrc" 2>/dev/null || source "$HOME/.zshrc" 2>/dev/null || true
fi
bun upgrade
bun --version

# 4. Install uv (Python package manager)
if ! command -v uv &> /dev/null; then
    echo "Installing uv..."
    curl -LsSf https://astral.sh/uv/install.sh | sh
fi
uv --version

# 5. Create Rust workspace structure
echo "Creating Rust workspace..."
mkdir -p rust-backend
cd rust-backend

# Create workspace Cargo.toml
cat > Cargo.toml << 'EOF'
[workspace]
members = ["crates/*"]

[workspace.package]
description = "FullStackHex project"
license = "MIT"
repository = "https://github.com/yourusername/fullstackhex"
authors = ["Your Name <your@email.com>"]

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

# Create crates directory
mkdir -p crates

# Create individual crates
for crate in api core db python-sidecar; do
    if [ ! -d "crates/$crate" ]; then
        echo "Creating crate: $crate"
        cargo new --lib --edition 2024 "crates/$crate"
    fi
done

# 6. Copy environment files
cd ..
echo "Setting up environment..."
if [ ! -f .env ]; then
    cp .env.example .env
fi

# 7. Configure Python sidecar socket path
if ! grep -q "PYTHON_SIDECAR_SOCKET" .env 2>/dev/null; then
    echo "" >> .env
    echo "# Python Sidecar (Unix socket)" >> .env
    echo "PYTHON_SIDECAR_SOCKET=/tmp/python-sidecar.sock" >> .env
fi

echo ""
echo "=== Initialization Complete ==="
echo ""
echo "Next steps:"
echo "  1. docker compose -f docker-compose.dev.yml up -d"
echo "  2. cd rust-backend && cargo run --workspace"
echo "  3. cd frontend && bun run dev"
echo ""
echo "Verify versions:"
echo "  rustc --version    (should show latest stable)"
echo "  bun --version       (should show latest)"
echo "  uv --version        (should show latest)"
echo ""
```

## Scaffold Astro Frontend

> **Note:** `scripts/install.sh` runs this automatically. The steps below are the manual equivalent.

```bash
# From repo root
bun create astro@latest frontend -- --template minimal --no-install --no-git --yes

cd frontend

# Add Tailwind integration (also installs dependencies)
bunx astro add tailwind --yes
```

Recommended template layout:

```text
frontend/
├── astro.config.mjs
├── package.json
├── tailwind.config.mjs
├── src/
│   ├── components/
│   ├── layouts/
│   └── pages/
│       ├── index.astro
│       └── api/
│           └── health.ts
└── public/
```

Use Astro server routes for backend-derived data:

```typescript
// src/pages/api/health.ts
export async function GET() {
    const response = await fetch(`${import.meta.env.VITE_RUST_BACKEND_URL}/health`);
    const body = await response.json();

    return new Response(JSON.stringify(body), {
        headers: { 'Content-Type': 'application/json' },
    });
}
```

## Make Script Executable

```bash
chmod +x scripts/install.sh
```

## Verification

```bash
./scripts/install.sh

# Verify versions
rustc --version    # Should show latest stable (edition 2024)
bun --version       # Should show latest
uv --version        # Should show latest

# Verify workspace
cd rust-backend
cargo build --workspace
ls crates/           # Should show: api core db python-sidecar

# Test Unix socket path exists in config
grep PYTHON_SIDECAR_SOCKET .env
```

## Portable Template

To use this as a template for new projects:

1. Copy the entire `scripts/install.sh` to your new project
2. Ensure the directory structure matches:
   ```
   your-project/
   ├── scripts/install.sh
   ├── rust-backend/ (empty, will be populated)
   │   └── crates/
   │       └── python-sidecar/ (your FastAPI sidecar crate)
   ├── frontend/ (your Astro project)
   └── .env.example (with PYTHON_SIDECAR_SOCKET)
   ```

3. Run `./scripts/install.sh` in your new project

## What Gets Installed (Latest Versions)

| Tool | Install Method | Version Check |
|------|----------------|--------------|
| Rust | rustup | `rustc --version` |
| Bun | Official script | `bun --version` |
| uv | Astral script | `uv --version` |
| Docker | Manual (if missing) | `docker --version` |

## Troubleshooting

### Script fails on Rust install
```bash
# Manual install
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### Bun not found after install
```bash
# Add to shell config
export BUN_INSTALL="$HOME/.bun"
export PATH="$BUN_INSTALL/bin:$PATH"
```

### Workspace build fails
```bash
cd rust-backend
cargo clean
cargo build --workspace
```

---

## Related Docs

- [Previous: INFRASTRUCTURE.md](./INFRASTRUCTURE.md) - Docker setup and config
- [All Docs](./INDEX.md) - Full documentation index
