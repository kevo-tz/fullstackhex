# Bare Metal Demo - High Performance Stack

A production-ready full-stack application with **Rust backend**, **Python services**, **Astro.js frontend (Bun SSR)**, and containerized infrastructure optimized for high performance.

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                    Nginx (HTTP/2, Compression)                     │
│                         Port 80/443                               │
└──────────────┬──────────────────────────┬─────────────────────────┘
               │                          │
     ┌─────────▼──────────┐    ┌─────────▼──────────┐
     │   Astro Frontend    │    │   Rust Backend     │
│ Port 3001 (Bun SSR)  │    │  Port 8001 (Axum)  │
     └─────────┬──────────┘    └─────────┬──────────┘
               │                          │
               ├──────────┬───────────────┤
               │          │               │
        ┌──────▼──────┐  │    ┌─────────▼──────────┐
        │  Python      │  │    │  Postgres (Single) │
        │  Services   │  │    │  Port 5432         │
        │  Port 8000  │  │    │  Schemas: rust, python │
        └──────┬──────┘  │    └────────────────────┘
               │          │
               ├──────────┤
               │          │
        ┌──────▼──────┐  ┌▼──────────────┐
        │    Redis     │  │   RustFS (S3)   │
        │  Port 6379  │  │  Port 9000     │
        └─────────────┘  └───────────────┘

┌─────────────────────────────────────────────────────────────────────┐
│              Monitoring: Prometheus + Grafana                       │
│                 Ports: 9090 (Prometheus), 3000 (Grafana)          │
└─────────────────────────────────────────────────────────────────────┘
```

## Tech Stack

| Component | Technology | Purpose |
|-----------|------------|---------|
| Frontend | Astro.js + Bun (SSR) | High-performance SSR with Bun runtime |
| Backend | Rust + Axum + Tokio | Async web server with HTTP/2 |
| Services | Python + FastAPI + Uvicorn | Rapid API development |
| Database | PostgreSQL 16 (Single instance + Schemas) | Unified data layer |
| Cache | Redis 7 (LRU eviction) | Sub-millisecond caching |
| Storage | RustFS (S3-compatible) | Object storage |
| Reverse Proxy | Nginx (HTTP/2 + Brotli) | TLS termination, compression |
| Monitoring | Prometheus + Grafana | Metrics, dashboards, alerting |

## Quick Start

### Prerequisites
- Docker & Docker Compose
- Rust (1.75+)
- Python (3.11+)
- Bun (1.0+)
- Node.js (for Astro CLI)

### Setup
```bash
# Run the setup script
./scripts/setup.sh

# Start all services (including monitoring)
docker-compose --profile production up -d

# Or start without Nginx (dev mode)
docker-compose up -d postgres redis rustfs
```

### Development
```bash
# Rust Backend (with hot reload via cargo-watch)
cd rust-backend
cargo watch -x run

# Python Services
cd python-services
uv run uvicorn src.main:app --reload --host 0.0.0.0 --port 8000

# Astro Frontend (Bun)
cd frontend
bun install
bun run dev  # Development with HMR
bun run build && bun run start  # Production SSR
```

### Monitoring
```bash
# Prometheus metrics
open http://localhost:9090

# Grafana dashboards (default: admin/admin)
open http://localhost:3000
```

## Performance Features

✅ **Async Everything**: Rust (Tokio), Python (asyncio), Redis (aio)  
✅ **Connection Pooling**: Postgres (50 max), Redis (manager)  
✅ **HTTP/2**: Enabled via Axum + Nginx  
✅ **Compression**: Brotli/Gzip via Nginx + Tower  
✅ **Caching**: Multi-layer (Redis + HTTP cache headers)  
✅ **Rate Limiting**: Tower governor middleware  
✅ **Observability**: OpenTelemetry + Prometheus metrics  
✅ **Resource Limits**: Docker containers with CPU/memory constraints  
✅ **Persistence**: Redis AOF, Postgres WAL  

## Project Structure

```
bare_metal_demo/
├── frontend/               # Astro.js + Bun SSR
│   ├── src/               # Astro components + pages
│   ├── astro.config.mjs   # Astro config with Bun adapter
│   └── package.json
├── rust-backend/           # Rust + Axum
│   ├── src/
│   │   ├── main.rs        # Entry point with middleware
│   │   ├── cache.rs      # Async Redis client
│   │   └── metrics.rs    # Prometheus metrics
│   └── Cargo.toml
├── python-services/        # Python + FastAPI
│   ├── src/
│   │   ├── main.py       # FastAPI app with instrumentation
│   │   └── cache.py     # Async Redis client
│   └── pyproject.toml
├── docker-compose.yml      # Production-grade orchestration
├── nginx/                  # Nginx config (HTTP/2, SSL)
├── postgres/               # Postgres config + init scripts
├── monitoring/             # Prometheus + Grafana config
├── scripts/               # Setup, verify, cleanup
└── docs/                  # Architecture + setup guides
```

## Configuration

### Environment Variables (`.env`)
```bash
# Database (Single Postgres instance)
DATABASE_URL=postgres://app_user:app_pass@localhost:5432/app_database

# Redis
REDIS_URL=redis://localhost:6379

# RustFS (S3-compatible)
RUSTFS_ENDPOINT=http://localhost:9000
RUSTFS_ACCESS_KEY=minioadmin
RUSTFS_SECRET_KEY=minioadmin

# Monitoring
PROMETHEUS_PORT=9090
GRAFANA_PORT=3000

# Frontend
NODE_ENV=production
ASTRO_PORT=3001
```

## Benchmarks (Example)

| Endpoint | Before (req/s) | After (req/s) | Improvement |
|----------|-----------------|---------------|-------------|
| Rust GET /health | 12,000 | 45,000 | 3.75x |
| Python GET /cache-test | 8,000 | 22,000 | 2.75x |
| Frontend SSR | N/A | 1,200 | New |

## Cleanup
```bash
# Stop all containers
docker-compose down

# Remove volumes (CAUTION: deletes data)
docker-compose down -v

# Full cleanup
./scripts/cleanup.sh
```

## License

See LICENSE file for details.
