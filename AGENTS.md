# AGENTS.md — Bare Metal Demo

Monorepo: 3 services (Rust, Python, Astro/Bun) + containerized infra.

## Services (ports verified from source code)
- `rust-backend/`: Axum, Tokio, port 8001 (edition 2024)
- `python-services/`: FastAPI, `uv` package manager, port 8000 (Python 3.14+)
- `frontend/`: Astro 6.x, Bun, port 4321 (Astro default)

Ignore docs with conflicting port numbers (e.g., `docs/SERVICES.md` is outdated).

## Commands
### Tests
- Frontend: `cd frontend && bun test` (single test: `bun test <file>`)
- Rust: `cd rust-backend && cargo test` (single test: `cargo test -p bare-metal-rust <test_name>`)
- Python: `cd python-services && uv run pytest` (single test: `uv run pytest <file>::<test_name>`)

### Dev Workflow (run in order)
1. Start infra: `docker compose -f docker-compose.dev.yml up -d` (Postgres, Redis, RustFS)
2. Run services (after infra is healthy):
   - Rust: `cd rust-backend && cargo run`
   - Python: `cd python-services && uv run uvicorn src.main:app --reload`
   - Frontend: `cd frontend && bun run dev`

## Setup
- Copy root `.env.example` to `.env` for infra config
- Rust backend uses `rust-backend/.env` for DB credentials
- Install dependencies: `./scripts/install.sh` (installs Rust, Bun, uv)

## References
- `CLAUDE.md`: Test commands, OpenCode skill routing rules
- `docs/`: Architecture, setup, and service details
