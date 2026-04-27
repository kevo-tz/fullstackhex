# System Architecture Overview

## Table of Contents

1. [Core Architecture](#core-architecture-rust-centric-with-python-sidecar)
2. [Key Architectural Decisions](#key-architectural-decisions)
3. [Data Flow](#data-flow)
4. [Technology Stack (Latest Versions)](#technology-stack-latest-versions)
5. [Workspace Structure](#workspace-structure)
6. [IPC: Unix Domain Socket](#ipc-unix-domain-socket)
7. [Port Mappings](#port-mappings)
8. [Related Docs](#related-docs)

## Core Architecture: Rust-Centric with Python Sidecar

```
┌────────────────────────────────────────────────────────┐
│          Frontend (Astro + Bun)                       │
│              Port 4321                                 │
└─────────────────┬──────────────────────────────────────┘
                  │ HTTP API only
                  ▼
┌────────────────────────────────────────────────────────┐
│         Rust Backend (Axum + Tokio)                   │
│              Port 8001 (only external API)            │
│                                                        │
│  Workspace Crates:                                    │
│  ├── api/          HTTP routes, middleware            │
│  ├── core/         Business logic                    │
│  ├── db/           sqlx + PostgreSQL                 │
│  └── python-sidecar/ Sidecar manager                │
│                        │                              │
│                        │ Unix domain socket            │
│                        ▼                              │
│  ┌─────────────────────────────────────────┐         │
│  │    Python Service (FastAPI)             │         │
│  │    /tmp/python-sidecar.sock (internal) │         │
│  └─────────────────────────────────────────┘         │
└──────────────┬───────────────────────────────────────┘
               │
    ┌──────────┼──────────┐
    ▼          ▼          ▼
┌────────┐ ┌────────┐ ┌─────────┐
│ Postgres│ │ Redis  │ │ RustFS  │
│  5432   │ │ 6379   │ │  9000   │
└────────┘ └────────┘ └─────────┘
```

### Production Additions (not active in dev)

```
┌─────────────────────────────────────────┐
│ Nginx (Reverse Proxy)                  │
│   :80 HTTP  /  :443 HTTPS              │
│   Sits in front of Frontend + Backend  │
└─────────────────────────────────────────┘

┌─────────────────────────────────────────┐
│ Monitoring Stack                       │
│  ├── Prometheus :9090  (scrapes all)   │
│  └── Grafana    :3000  (dashboards)    │
└─────────────────────────────────────────┘
```

## Key Architectural Decisions

| Decision | Rationale |
|----------|-----------|
| Python as Rust sidecar | Single entry point, Rust controls lifecycle |
| Frontend → Rust only | Simplified networking, Rust proxies to Python |
| Rust workspace | Modular crates, clear boundaries |
| Unix domain socket | Fast IPC on Linux/macOS, no TCP overhead |
| Latest versions | Predictable initialization via scripts |

## Data Flow

1. **Browser** → Requests Astro frontend on `localhost:4321`
2. **Astro Server Route** → Calls Rust backend on `localhost:8001` when backend data needed
3. **Rust API** → Processes request, may call internal crates
4. **Python Sidecar** → Rust communicates via Unix domain socket when Python logic needed
5. **Data Layer** → Postgres (sqlx) + Redis + RustFS
6. **Production only** → Nginx terminates TLS and proxies external traffic; Prometheus + Grafana monitor the entire stack

## Technology Stack (Latest Versions)

| Component | Technology | Version Check Command |
|-----------|-------------|----------------------|
| Rust | Edition 2024 | `rustc --version` |
| Workspace | Cargo workspace | Auto |
| Web Framework | Axum 0.8+ | Check crates.io |
| Async Runtime | Tokio 1.x | Check crates.io |
| Python | 3.14+ | `python3 --version` |
| Package Manager | uv (latest) | `uv --version` |
| Frontend | Astro 6.x + Bun | `bun --version` |
| Database | PostgreSQL 18 | `docker compose ps` |
| Cache | Redis 8 | `docker compose ps` |
| Object Storage | RustFS (S3-compatible) | `docker compose ps` |
| Monitoring | Prometheus 3.x + Grafana | Production only, see .env.example |
| Reverse Proxy | Nginx (production) | Production only, see .env.example |
| IPC | Unix domain socket | `/tmp/python-sidecar.sock` |

## Workspace Structure

```
frontend/
├── astro.config.mjs         # Astro SSR config (output: server, @astrojs/node adapter)
├── package.json             # Bun-managed scripts and dependencies
├── tsconfig.json            # TypeScript config (extends astro/tsconfigs/strict)
├── src/
│   └── pages/
│       ├── index.astro      # Template landing page
│       └── api/
│           └── health.ts    # Astro server route proxying Rust health
└── tests/                   # Bun test suites (unit, integration, smoke)

backend/
├── Cargo.toml              # Workspace root
├── crates/
│   ├── api/               # HTTP API layer (Axum routes)
│   ├── core/              # Business logic
│   ├── db/                # Database layer (sqlx)
│   └── python-sidecar/    # Sidecar process manager
└── target/
```

## IPC: Unix Domain Socket

Python sidecar binds to `/tmp/python-sidecar.sock`. Rust communicates through this socket for:
- Low latency (no TCP overhead)
- Security (only local processes can connect)
- Simple integration with FastAPI/Uvicorn

## Port Mappings

| Service | Port | Purpose |
|---------|------|---------|
| Frontend | 4321 | Development server |
| Rust Backend | 8001 | Only external API |
| Python Sidecar | Internal | Unix socket only (/tmp/python-sidecar.sock) |
| PostgreSQL | 5432 | Database |
| Redis | 6379 | Cache |
| RustFS | 9000 | S3-compatible storage |
| RustFS | 9001 | Console for storage |
| Prometheus | 9090 | Metrics collection (production) |
| Grafana | 3000 | Monitoring dashboards (production) |
| Nginx | 80/443 | Reverse proxy (production) |


## Related Docs

- [Previous: SETUP.md](./SETUP.md) - One-command init and tool install
- [Next: SERVICES.md](./SERVICES.md) - Service details and communication
- [All Docs](./INDEX.md) - Full documentation index
