# Project Initialization Template

Use this as a portable template for new projects with the same Rust + Bun + Python sidecar architecture.

> **Note for this repo:** all source files already ship committed. Run `make setup` to install tools and create `.env`. See [SETUP.md](./SETUP.md) for the full guide.

## Table of Contents

1. [Prerequisites](#prerequisites)
2. [Initialization Script](#initialization-script)
3. [Scaffold Astro Frontend](#scaffold-astro-frontend)
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

Below is a minimal portable bootstrap for a new project. Adjust paths and names as needed.

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
mkdir -p backend
cd backend

# Create workspace Cargo.toml
cat > Cargo.toml << 'EOF'
[workspace]
members = ["crates/*"]
resolver = "3"

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
for crate in api auth cache db domain python-sidecar storage; do
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
echo "  1. docker compose -f compose/dev.yml up -d"
echo "  2. cd backend && cargo run -p api"
echo "  3. cd frontend && bun run dev"
echo ""
echo "Verify versions:"
echo "  rustc --version    (should show latest stable)"
echo "  bun --version       (should show latest)"
echo "  uv --version        (should show latest)"
echo ""
```

## Scaffold Astro Frontend

```bash
# From repo root
bun create astro@latest frontend -- --template minimal --no-install --no-git --yes

cd frontend

# Install Tailwind v4 (vite plugin) and Node SSR adapter
bun add @tailwindcss/vite tailwindcss @astrojs/node
bun install
```

Recommended template layout:

```text
frontend/
├── astro.config.mjs
├── package.json
├── tsconfig.json
├── src/
│   ├── components/
│   ├── layouts/
│   └── pages/
│       ├── index.astro
│       └── api/
│           └── health.ts
└── public/
```

> **Note:** No `tailwind.config.mjs` — Tailwind v4 is configured via the vite plugin in `astro.config.mjs`.

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

## Verification

```bash
chmod +x scripts/install.sh
./scripts/install.sh

# Verify versions
rustc --version    # Should show latest stable (edition 2024)
bun --version       # Should show latest
uv --version        # Should show latest

# Verify workspace
cd backend
cargo build --workspace
ls crates/           # Should show: api auth cache db domain python-sidecar storage

# Verify generated tests
cargo test --workspace

# Verify Python sidecar scaffold and tests
cd ../python-sidecar
uv sync --all-extras
uv run pytest

# Verify frontend tests
cd ../frontend
bun test

# Test Unix socket path exists in config
cd ..
grep PYTHON_SIDECAR_SOCKET .env
```

## Portable Template

To use this architecture for a new project:

1. Copy `scripts/install.sh` (from the Initialization Script section above) to your new project
2. Ensure the skeleton directory structure:
   ```
   your-project/
   ├── scripts/install.sh
   ├── .env.example
   └── compose/
   ```
3. Run `chmod +x scripts/install.sh && ./scripts/install.sh`

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
cd backend
cargo clean
cargo build --workspace
```

---

## Related Docs

- [Previous: INFRASTRUCTURE.md](./INFRASTRUCTURE.md) - Docker setup and config
- [All Docs](./INDEX.md) - Full documentation index
