# System Architecture Overview

## Table of Contents

1. [Core Architecture](#core-architecture-rust-centric-with-py-api)
2. [Key Architectural Decisions](#key-architectural-decisions)
3. [Data Flow](#data-flow)
4. [Technology Stack (Latest Versions)](#technology-stack-latest-versions)
5. [Workspace Structure](#workspace-structure)
6. [IPC: Unix Domain Socket](#ipc-unix-domain-socket)
7. [Port Mappings](#port-mappings)
8. [Related Docs](#related-docs)

## Core Architecture: Rust-Centric with py-api

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
│  ├── auth/         JWT + OAuth + CSRF authentication │
│  ├── cache/        Redis caching, rate limiting      │
│  ├── db/           sqlx + PostgreSQL                 │
│  ├── domain/       Business logic and shared types   │
│  ├── py-sidecar/   Unix socket client for Python IPC │
│  └── storage/      S3-compatible object storage      │
│                        │                              │
│                        │ Unix domain socket            │
│                        ▼                              │
│  ┌─────────────────────────────────────────┐         │
│  │    Python Service (FastAPI)             │         │
│  │    Dev: /tmp/fullstackhex-python.sock   │         │
│  │    Prod: /tmp/sidecar/py-api.sock      │         │
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
| Python as Rust sidecar | Single entry point, Python runs independently alongside Rust |
| Frontend → Rust only | Simplified networking, Rust proxies to Python |
| Rust workspace | Modular crates, clear boundaries |
| Unix domain socket | Fast IPC on Linux/macOS, no TCP overhead |
| Latest versions | Predictable initialization via scripts |
| Auth in Rust crate | JWT, OAuth, session logic lives in the backend, not scattered |
| Redis for sessions | Session store outside PostgreSQL reduces load on the primary db |
| S3-compatible storage | Portable across RustFS, MinIO, AWS S3, Cloudflare R2 |

## Data Flow

1. **Browser** → Requests Astro frontend on `localhost:4321`
2. **Astro Server Route** → Calls Rust backend on `localhost:8001` when backend data needed
3. **Rust API** → Processes request, may call internal crates
4. **py-api** → Rust communicates via Unix domain socket when Python logic needed
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
| IPC | Unix domain socket | `/tmp/fullstackhex-python.sock` (dev), `/tmp/sidecar/py-api.sock` (prod) |

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
├── api/               # HTTP API layer (Axum routes)
├── auth/              # JWT + OAuth + CSRF authentication
├── cache/             # Redis caching, rate limiting, sessions
├── db/                # Database layer (sqlx)
├── domain/            # Business logic and shared types
├── py-sidecar/        # Unix socket client for Python IPC
└── storage/           # S3-compatible object storage
└── target/
```

## IPC: Unix Domain Socket

py-api binds to `/tmp/fullstackhex-python.sock` in dev and `/tmp/sidecar/py-api.sock` in prod. Rust communicates through this socket for:
- Low latency (no TCP overhead)
- Security (only local processes can connect)
- Simple integration with FastAPI/Uvicorn

### PythonSidecar (implemented in v0.3.1.0)

The \`PythonSidecar\` struct in \`backend/py-sidecar/src/lib.rs\` handles
HTTP communication with a running py-api process via a Unix domain socket. The socket
path is `/tmp/fullstackhex-python.sock` in dev and `/tmp/sidecar/py-api.sock` in prod.
Start it with `uv run uvicorn app.main:app --uds /tmp/fullstackhex-python.sock` (or use `make dev` to start everything).

```rust
// Key API:
// - PythonSidecar::new(path, timeout, max_retries) — explicit configuration
// - PythonSidecar::from_env() — reads PYTHON_SIDECAR_SOCKET, PYTHON_SIDECAR_TIMEOUT_MS, etc.
// - is_available() — checks socket file existence
// - get(path) — HTTP GET over Unix socket with retry + timeout
// - health() — convenience: get("/health")

// Error types: SocketNotFound, ConnectionFailed, Timeout, InvalidResponse, HttpError
// Retry: 3 attempts with exponential backoff (100ms, 200ms, 400ms)
// Timeout: 5s per attempt (configurable via PYTHON_SIDECAR_TIMEOUT_MS)
// Always returns HTTP 200 — service status is in the JSON body
```

### Database Health (implemented in v0.3.1.0)

The \`db\` crate (\`backend/db/src/lib.rs\`) exports `health_check(pool: Option<&PgPool>)`
which runs `SELECT 1` against PostgreSQL. A 3-second timeout prevents hanging on a
slow database. The api crate uses it in the `/health/db` handler.

### Configuration via Environment

```rust
// Read socket path from environment
fn get_socket_path() -> PathBuf {
    std::env::var("PYTHON_SIDECAR_SOCKET")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp/fullstackhex-python.sock"))
}
```

## Port Mappings

| Service | Port | Purpose |
|---------|------|---------|
| Frontend | 4321 | Development server |
| Rust Backend | 8001 | Only external API |
| py-api | Internal | Dev: /tmp/fullstackhex-python.sock, Prod: /tmp/sidecar/py-api.sock |
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
