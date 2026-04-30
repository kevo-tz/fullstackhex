# Agent Instructions — FullStackHex

## Dev Start Order (do this sequence)

```bash
make up                      # starts PostgreSQL, Redis, RustFS + monitoring overlay
cd backend && cargo run -p api  # backend + auto-spawns Python sidecar
cd frontend && bun run dev      # Astro dev server on :4321
```

## Test Commands

```bash
make test-rust         # cd backend && cargo test --workspace
make test-python       # cd python-sidecar && uv run pytest
make test-frontend   # cd frontend && bun test
```

## Code Quality

```bash
# Rust
cd backend && cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings

# Python
cd python-sidecar && uv run ruff check . && uv run ruff format .

# Frontend
cd frontend && bun run build
```

## Rust Backend Entry

- Run via `cargo run -p api` from `backend/` (NOT `cargo run --workspace`)
- Rust auto-spawns Python sidecar at `/tmp/python-sidecar.sock`
- Port 8001 is the only external API port; frontend never calls Python directly

## Frontend

- Astro SSR with `output: 'server'` and `@astrojs/node` adapter
- Tailwind v4: no `tailwind.config.mjs` — configured via `@tailwindcss/vite` plugin in `astro.config.mjs`
- `VITE_RUST_BACKEND_URL=http://localhost:8001` is the API base for server routes

## Docker Infra

```bash
make up        # compose/dev.yml + compose/monitor.yml
make down     # stops everything
make clean    # removes volumes too
docker compose -f compose/dev.yml ps  # check status
```

Ports: PostgreSQL :5432, Redis :6379, RustFS :9000, Grafana :3000

## LeanKG

MCP server available at `http://localhost:3000`. Config in `leankg.yaml`.

## Branch Naming

`feat/<name>`, `fix/<name>`, `docs/<name>`, `refactor/<name>`, `infra/<name>`

## Key Env Vars

```env
DATABASE_URL=postgres://app_user:...@localhost:5432/app_database
PYTHON_SIDECAR_SOCKET=/tmp/python-sidecar.sock
VITE_RUST_BACKEND_URL=http://localhost:8001
```