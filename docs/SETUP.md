# Setup Guide

## Table of Contents

1. [Prerequisites](#prerequisites)
2. [First-Time Setup](#first-time-setup)
3. [Start Development](#start-development)
4. [Verify Installation](#verify-installation)
5. [Environment Configuration](#environment-configuration)
6. [Troubleshooting](#troubleshooting)
7. [Related Docs](#related-docs)

## Prerequisites

All source code, configs, and test files ship in the repo — no scaffolding step required. You only need the runtime tools installed.

| Tool | Version | Install |
|------|---------|---------|
| Rust | stable (edition 2024) | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| Bun | latest | `curl -fsSL https://bun.sh/install \| bash` |
| Python | 3.14+ | via pyenv: `pyenv install 3.14-dev` |
| uv | latest | `curl -LsSf https://astral.sh/uv/install.sh \| sh` |
| Docker + Compose | any recent | [docs.docker.com/get-docker](https://docs.docker.com/get-docker/) |

> **Python 3.14** is pre-release on most systems. Install via pyenv:
> ```bash
> curl https://pyenv.run | bash
> pyenv install 3.14-dev
> pyenv global 3.14-dev
> ```

## First-Time Setup

```bash
git clone <repo>
cd fullstackhex

# Create .env from template
cp .env.example .env
```

## Start Development

```bash
# 1. Start infrastructure (PostgreSQL, Redis, RustFS)
docker compose -f compose/dev.yml up -d

# 2. (Optional) Start monitoring stack (Prometheus + Grafana)
docker compose -f compose/monitor.yml up -d

# 3. Start Rust API (in a separate terminal)
cd backend && cargo run -p api

# 4. Start Astro frontend (in a separate terminal)
cd frontend && bun run dev
```

Ports:

| Service | URL |
|---------|-----|
| Rust API | http://localhost:8001 |
| Frontend | http://localhost:4321 |
| Grafana | http://localhost:3000 (requires monitor.yml) |
| PostgreSQL | localhost:5432 |
| Redis | localhost:6379 |

The Python sidecar runs as an independent process alongside Rust. Start it together via `make dev` or `make watch`, or manually with `cd py-api && uv run uvicorn app.main:app --uds /tmp/fullstackhex-python.sock`.

## Verify Installation

```bash
# All services healthy
make status

# Individual checks
curl http://localhost:8001/health
curl http://localhost:8001/health/python
curl http://localhost:4321

# Infrastructure
docker compose -f compose/dev.yml ps
```

## Environment Configuration

`.env` is created from `.env.example`. Key variables:

```env
# Database
DATABASE_URL=postgres://app_user:CHANGE_ME@localhost:5432/app_database

# py-api (Unix socket)
PYTHON_SIDECAR_SOCKET=/tmp/fullstackhex-python.sock

# Frontend → Rust
VITE_RUST_BACKEND_URL=http://localhost:8001
ASTRO_PORT=4321
PUBLIC_API_URL=http://localhost:8001
```

Replace `CHANGE_ME` with a real password before running `docker compose -f compose/dev.yml up -d`.

## Troubleshooting

### Port conflicts

```bash
lsof -i :8001   # or :4321, :5432, :6379
# Change ports in .env and compose/dev.yml
```

### Rust build errors

```bash
cd backend
cargo clean && cargo build --workspace
```

### Python dependencies

```bash
cd py-api
uv sync
```

### Infrastructure issues

```bash
docker compose -f compose/dev.yml logs postgres
docker compose -f compose/dev.yml logs redis
docker compose -f compose/dev.yml restart
```

### Socket path issues

`PYTHON_SIDECAR_SOCKET` in `.env` must point to a path the current user can write to. Default is `/tmp/fullstackhex-python.sock`.

## Related Docs

- [ARCHITECTURE.md](./ARCHITECTURE.md) — System design and data flow
- [CI.md](./CI.md) — GitHub Actions pipeline
- [All Docs](./INDEX.md) — Full documentation index
