# Project Initialization Template

Use this as a portable template for new projects with the same Rust + Bun + Python API architecture.

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

Use `install.sh` at the repo root to scaffold a new project:

```bash
./install.sh my-new-project
```

Flags:
- `--dry-run` — preview actions without executing
- `--skip-deps` — skip `uv sync` and `bun install`
- `--skip-git` — skip `git init` and initial commit
- `--skip-verify` — skip `cargo check` and `bun run typecheck`

The script validates tools (Cargo, Bun, uv, Docker), copies the template, installs Python 3.14 via uv, configures project names, installs dependencies, and creates a git commit.

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
./install.sh my-test-project --skip-deps --skip-git

# Verify workspace
cd my-test-project/backend
cargo build --workspace
ls -d */           # Should show: api auth cache db domain py-sidecar storage

# Verify tests
cargo test --workspace

# Verify Python API
cd ../py-api
uv sync --all-extras
uv run pytest

# Verify frontend
cd ../frontend
bun test

# Verify config
cd ..
grep APP_NAME .env
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
