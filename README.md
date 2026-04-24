# Bare Metal Template - High Performance Stack

A minimal, high-performance development template with **Rust**, **Python**, **Bun**, and containerized infrastructure.

## Architecture

```
┌─────────────────────────────────────────────────┐
│            Nginx (Production)                   │
│              Port 80/443                        │
└────────┬──────────────┬────────────────────────┘
         │              │
    ┌────▼─────┐  ┌────▼─────┐
     │  Frontend │  │  Rust    │
     │  Astro+Bun│  │  Axum    │
     │  Port 4321│  │  Port 8001│
    └───────────┘  └────┬─────┘
                         │
              ┌──────────┼──────────┐
              │          │          │
         ┌────▼─────┐ ┌▼────────┐ ┌▼────────┐
         │  Python  │ │ Postgres│ │  Redis  │
         │  FastAPI │ │ Port 5432│ │ Port 6379│
         │  Port 8000│ └─────────┘ └─────────┘
         └───────────┘
```

## Tech Stack

| Component | Technology | Purpose |
|-----------|------------|---------|
| Frontend | Astro.js + Bun | High-performance SSR |
| Backend | Rust + Axum | Async web server |
| Services | Python + FastAPI | Rapid API development |
| Database | PostgreSQL 16 | Primary datastore |
| Cache | Redis 7 | Sub-millisecond caching |
| Storage | RustFS | S3-compatible object storage |
| Proxy | Nginx | Reverse proxy (production) |
| Monitoring | Prometheus + Grafana | Metrics + dashboards |

## Quick Start

### Install Dependencies

```bash
# Check and install missing dependencies (Rust, Bun, uv)
./scripts/install.sh
```

### Development (Infrastructure Only)

```bash
# Start Postgres, Redis, RustFS
docker compose -f docker-compose.dev.yml up -d

# Run services locally:
cd rust-backend && cargo run              # Port 8001
cd python-services && uv run uvicorn src.main:app --reload  # Port 8000
cd frontend && bun run dev                 # Port 4321
```

### Production (Full Stack)

```bash
# Start all services including monitoring
docker compose -f docker-compose.prod.yml up -d

# Access:
# Frontend:  http://localhost
# Rust API:  http://localhost:8001
# Python API: http://localhost:8000
# Prometheus: http://localhost:9090
# Grafana:    http://localhost:3000
```

## Project Structure

```
bare-metal-template/
├── frontend/               # Astro.js + Bun
│   └── src/pages/index.astro
├── rust-backend/           # Rust + Axum
│   └── src/main.rs
├── python-services/        # Python + FastAPI
│   └── src/main.py
├── docker-compose.dev.yml  # Infrastructure only
├── docker-compose.prod.yml # Full stack + monitoring
├── nginx/                  # Nginx config
├── monitoring/             # Prometheus + Grafana
├── scripts/
│   └── install.sh         # Dependency installer
└── README.md
```

## Endpoints

| Service | Endpoint | Response |
|---------|-----------|----------|
| Rust | `GET /` | `{"message": "Hello from Rust!"}` |
| Rust | `GET /health` | `{"status": "healthy"}` |
| Python | `GET /` | `{"message": "Hello from Python!"}` |
| Python | `GET /health` | `{"status": "healthy"}` |
| Frontend | `GET /` | HTML page |

## License

MIT
