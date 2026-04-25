# Setup Guide - Rust/Bun/uv Latest-Version Stack

## Table of Contents

1. [One-Command Initialization](#one-command-initialization)
2. [What `install.sh` Does](#what-installsh-does)
3. [Manual Step-by-Step (Alternative)](#manual-step-by-step-alternative)
4. [Verify Installation](#verify-installation)
5. [Environment Configuration](#environment-configuration)
6. [Troubleshooting](#troubleshooting)
7. [Related Docs](#related-docs)

## One-Command Initialization

```bash
# Clone and run full initialization
git clone <repo>
cd bare-metal-demo
./scripts/install.sh
```

The script installs latest versions AND creates the Rust workspace structure.

## What `install.sh` Does

1. **Installs latest tools:**
   - Rust (edition 2024) via rustup
   - Bun (latest) via official installer
   - uv (latest Python package manager)
   - Docker & Docker Compose (if missing)

2. **Creates Rust workspace:**
   ```
   rust-backend/
   ├── Cargo.toml (workspace root)
   ├── crates/
   │   ├── api/
   │   ├── core/
   │   ├── db/
   │   └── python-sidecar/
   └── target/
   ```

3. **Sets up environment:**
   - Copies `.env.example` to `.env`
   - Configures Unix socket path for Python sidecar

## Manual Step-by-Step (Alternative)

### 1. Install Tools (Latest Versions)

```bash
# Rust (edition 2024)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup update stable
rustc --version  # Verify edition 2024

# Bun (latest)
curl -fsSL https://bun.sh/install | bash
bun upgrade
bun --version

# uv (latest Python package manager)
curl -LsSf https://astral.sh/uv/install.sh | sh
uv --version

# Verify Docker
docker --version
docker compose version
```

### 2. Create Rust Workspace

```bash
cd rust-backend

# Initialize workspace (done by install.sh)
cat > Cargo.toml << 'EOF'
[workspace]
members = ["crates/*"]

[workspace.dependencies]
tokio = { version = "1", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
axum = "0.8"
sqlx = { version = "0.8", features = ["postgres"] }
EOF

# Create crates
mkdir -p crates
for crate in api core db python-sidecar; do
    if [ ! -d "crates/$crate" ]; then
        cargo new --lib "crates/$crate"
    fi
done

# Build workspace
cargo build --workspace
```

### 3. Start Infrastructure

```bash
docker compose -f docker-compose.dev.yml up -d
docker compose ps
```

### 4. Run Services

```bash
# Terminal 1: Rust (with Python sidecar)
cd rust-backend
cargo run --workspace

# Terminal 2: Frontend
cd frontend
bun install
bun run dev
```

## Verify Installation

```bash
# Rust backend (with sidecar)
curl http://localhost:8001/health

# Python sidecar (via Rust, internal socket)
curl http://localhost:8001/api/python/health

# Frontend
curl http://localhost:4321

# Infrastructure
docker compose ps
```

## Environment Configuration

```bash
# Copy template
cp .env.example .env

# Review settings (defaults work for local dev)
cat .env
```

Key settings in `.env`:
```
# Rust Backend
RUST_SERVICE_DB_URL=postgres://localhost/bare_metal

# Python Sidecar (Unix socket)
PYTHON_SIDEcar_SOCKET=/tmp/python-sidecar.sock

# Frontend (Rust API only)
VITE_RUST_BACKEND_URL=http://localhost:8001
```

## Troubleshooting

### Port Conflicts
```bash
# Check what's using a port
lsof -i :5432

# Change ports in .env and docker-compose.yml
```

### Rust Build Errors
```bash
cd rust-backend
cargo clean
cargo build --workspace
```

### Python Dependencies
```bash
cd python-services
uv sync
```

### Infrastructure Issues
```bash
# Check logs
docker compose logs postgres
docker compose logs redis

# Restart services
docker compose restart
```

## Related Docs

- [Next: ARCHITECTURE.md](./ARCHITECTURE.md) - System design overview
- [All Docs](./INDEX.md) - Full documentation index
