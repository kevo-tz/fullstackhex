# System Architecture Overview

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

## Key Architectural Decisions

| Decision | Rationale |
|----------|-----------|
| Python as Rust sidecar | Single entry point, Rust controls lifecycle |
| Frontend → Rust only | Simplified networking, Rust proxies to Python |
| Rust workspace | Modular crates, clear boundaries |
| Unix domain socket | Fast IPC on Linux/macOS, no TCP overhead |
| Latest versions | Predictable initialization via scripts |

## Data Flow

1. **Frontend** → HTTP request to Rust (localhost:8001) only
2. **Rust API** → Processes request, may call internal crates
3. **Python Sidecar** → Rust communicates via Unix domain socket when Python logic needed
4. **Data Layer** → Postgres (sqlx) + Redis + RustFS

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
| IPC | Unix domain socket | `/tmp/python-sidecar.sock` |

## Workspace Structure

```
rust-backend/
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

## Next Steps

- See [SETUP.md](./SETUP.md) for installation instructions
- See [SERVICES.md](./SERVICES.md) for service details
- See [INITIALIZATION.md](./INITIALIZATION.md) for template-ready setup
