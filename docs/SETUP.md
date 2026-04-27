# Setup Guide - Rust/Bun/uv Latest-Version Stack

## Table of Contents

1. [One-Command Initialization](#one-command-initialization)
2. [What `install.sh` Does](#what-installsh-does)
3. [Manual Step-by-Step (Alternative)](#manual-step-by-step-alternative)
4. [Scaffold Frontend (Astro + Bun)](#scaffold-frontend-astro--bun)
5. [Verify Installation](#verify-installation)
6. [Environment Configuration](#environment-configuration)
7. [Troubleshooting](#troubleshooting)
8. [Related Docs](#related-docs)

## One-Command Initialization

```bash
# Clone and run full initialization
git clone <repo>
cd fullstackhex
mkdir -p backend
./scripts/install.sh
```

The script installs/updates Rust, Bun, and uv, validates Docker prerequisites, scaffolds `backend/`, scaffolds `python-sidecar/`, scaffolds `frontend/`, and generates baseline Rust/Python/frontend tests.

## What `install.sh` Does

1. **Checks and installs tools (in order):**
   - Rust (edition 2024) via rustup
   - Bun (latest) via official installer
   - Python 3.14+ validation (script exits if not found; install via pyenv first)
   - uv (latest Python package manager)
   - Docker & Docker Compose validation (script exits if not found; install manually)

2. **Creates or updates Rust workspace:**
   ```
   backend/
   ├── Cargo.toml (workspace root)
   ├── crates/
   │   ├── api/
   │   ├── core/
   │   ├── db/
   │   └── python-sidecar/
   └── target/
   ```

3. **Sets up environment:**
   - Creates `.env` (from `.env.example` if present, or empty)
   - Configures Unix socket path for Python sidecar (`PYTHON_SIDECAR_SOCKET`)
   - Adds `VITE_RUST_BACKEND_URL=http://localhost:8001`

4. **Scaffolds Astro frontend** (automated, idempotent):
   - Runs `bun create astro@latest frontend` (non-interactive, `--template minimal`)
   - Installs Tailwind v4 (`@tailwindcss/vite`) and Node SSR adapter (`@astrojs/node`)
   - Writes `astro.config.mjs` with `output: 'server'` and Tailwind vite plugin
   - Creates `src/pages/api/health.ts` proxy route to Rust backend

5. **Scaffolds Python sidecar** (automated, idempotent):
   - Creates `python-sidecar/pyproject.toml`
   - Creates `python-sidecar/app/main.py`
   - Creates `python-sidecar/tests/` with starter unit + integration tests

6. **Scaffolds generated test suites**:
   - Rust: starter unit/integration/smoke tests under crate `tests/` folders
   - Python: starter unit/integration tests under `python-sidecar/tests/`
   - Frontend: starter unit/integration/smoke tests under `frontend/tests/`

## Manual Step-by-Step (Alternative)

### 1. Install Tools (Latest Versions)

**Prerequisite:** Python 3.14+ required (check: `python3 --version`). Install via pyenv or package manager if missing.

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
mkdir -p backend
cd backend

# Initialize workspace (done by install.sh)
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

[profile.release]
lto = true
EOF

# Create crates
mkdir -p crates
for crate in api core db python-sidecar; do
    if [ ! -d "crates/$crate" ]; then
        cargo new --lib --edition 2024 "crates/$crate"
    fi
done

# Build workspace
cargo build --workspace
```

### 3. Start Infrastructure

```bash
# Verify Docker daemon is running
docker info > /dev/null 2>&1 || { echo "Docker daemon not running. Start Docker first."; exit 1; }

docker compose -f compose/dev.yml up -d
docker compose -f compose/dev.yml ps
```

### 4. Run Services

```bash
# Terminal 1: Rust API
cd backend
cargo run -p api

# Terminal 2: Frontend (dependencies already installed by install.sh)
cd frontend
bun run dev
```

## Scaffold Frontend (Astro + Bun)

> **Note:** `install.sh` runs this automatically. The steps below are the manual equivalent.

```bash
# From repo root
bun create astro@latest frontend -- --template minimal --no-install --no-git --yes

cd frontend

# Install Tailwind v4 (vite plugin) and Node SSR adapter
bun add @tailwindcss/vite tailwindcss @astrojs/node
bun install
```

Recommended first-page structure:

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

> **Note:** No `tailwind.config.mjs` — Tailwind v4 is configured entirely via the vite plugin.

Recommended first route implementation:

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

## Verify Installation

```bash
# Rust backend (with sidecar)
curl http://localhost:8001/health

# Frontend build
cd frontend
bun run build

# Python sidecar (via Rust, internal socket)
curl http://localhost:8001/api/python/health

# Frontend
curl http://localhost:4321

# Infrastructure
docker compose -f compose/dev.yml ps
```

## Environment Configuration

```bash
# Copy template
cp .env.example .env

# Review settings (defaults work for local dev)
cat .env
```

Key settings in `.env`:
```env
# Rust Backend
DATABASE_URL=postgres://app_user:CHANGE_ME@localhost:5432/app_database # pragma: allowlist secret

# Python Sidecar (Unix socket)
PYTHON_SIDECAR_SOCKET=/tmp/python-sidecar.sock

# Frontend (Rust API only)
ASTRO_PORT=4321
PUBLIC_API_URL=http://localhost:8001
VITE_RUST_BACKEND_URL=http://localhost:8001
```

## Troubleshooting

### Port Conflicts
```bash
# Check what's using a port
lsof -i :5432

# Change ports in .env and compose/dev.yml
```

### Rust Build Errors
```bash
cd backend
cargo clean
cargo build --workspace
```

### Python Dependencies

Python dependencies are managed in the root `python-sidecar/` project.

```bash
cd python-sidecar
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
