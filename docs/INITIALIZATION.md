# Project Initialization Template

Use this as a template for new projects with the same architecture.

## Table of Contents

1. [Prerequisites](#prerequisites)
2. [Initialization Script](#initialization-script)
3. [Make Script Executable](#make-script-executable)
4. [Verification](#verification)
5. [Portable Template](#portable-template)
6. [What Gets Installed (Latest Versions)](#what-gets-installed-latest-versions)
7. [Troubleshooting](#troubleshooting)
8. [Related Docs](#related-docs)

## Prerequisites

- Linux/macOS (Unix domain socket requires Unix-like OS)
- Git
- Internet connection

## Initialization Script

Save as `scripts/install.sh`:

```bash
#!/bin/bash
set -e

echo "=== Bare Metal Demo Initialization ==="

# 1. Install Rust (edition 2024)
if ! command -v rustc &> /dev/null; then
    echo "Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
fi
rustup update stable
rustc --version

# 2. Install Bun
if ! command -v bun &> /dev/null; then
    echo "Installing Bun..."
    curl -fsSL https://bun.sh/install | bash
    source "$HOME/.bashrc" 2>/dev/null || source "$HOME/.zshrc" 2>/dev/null || true
fi
bun upgrade
bun --version

# 3. Install uv (Python package manager)
if ! command -v uv &> /dev/null; then
    echo "Installing uv..."
    curl -LsSf https://astral.sh/uv/install.sh | sh
fi
uv --version

# 4. Create Rust workspace structure
echo "Creating Rust workspace..."
cd rust-backend

# Create workspace Cargo.toml
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
EOF

# Create crates directory
mkdir -p crates

# Create individual crates
for crate in api core db python-sidecar; do
    if [ ! -d "crates/$crate" ]; then
        echo "Creating crate: $crate"
        cargo new --lib "crates/$crate"
    fi
done

# 5. Copy environment files
cd ..
echo "Setting up environment..."
if [ ! -f .env ]; then
    cp .env.example .env
fi

# 6. Configure Python sidecar socket path
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
echo "  3. cd frontend && bun install && bun run dev"
echo ""
echo "Verify versions:"
echo "  rustc --version    (should show latest stable)"
echo "  bun --version       (should show latest)"
echo "  uv --version        (should show latest)"
echo ""
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
   ├── frontend/ (your Astro project)
   ├── python-services/ (your FastAPI project)
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
