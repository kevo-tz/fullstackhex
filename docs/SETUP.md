# Setup Guide

Complete guide to setting up the Bare Metal Demo high-performance stack.

## Prerequisites

- **Docker & Docker Compose** (v2.0+)
- **Rust** (edition 2024)
- **Python** (3.14+)
- **Bun** (1.0+)
- **Node.js** (18+ for Astro CLI)

## Quick Start

### 1. Clone & Setup Environment

```bash
# Copy environment template
cp .env.example .env

# Edit if needed (defaults work for local dev)
nano .env
```

### 2. Start Infrastructure

```bash
# Start all core services
docker-compose up -d postgres redis rustfs

# Verify services are healthy
docker-compose ps

# Check logs
docker-compose logs -f postgres
```

### 3. Run Services (Development Mode)

**Rust Backend:**
```bash
cd rust-backend
cargo run
# Or with hot reload (requires cargo-watch):
# cargo watch -x run
```

**Python Services:**
```bash
cd python-services
uv sync  # Install dependencies with uv
uv run uvicorn src.main:app --reload --host 0.0.0.0 --port 8000 --loop uvloop
```

**Astro Frontend:**
```bash
cd frontend
bun install
bun run dev  # Development with HMR on port 4321
```

### 4. Start Monitoring Stack

```bash
# Start Prometheus + Grafana
docker-compose --profile production up -d prometheus grafana

# Access:
# Prometheus: http://localhost:9090
# Grafana: http://localhost:3000 (admin/admin)
```

### 5. Production Deployment

```bash
# Build all images
docker-compose build

# Start all services (including Nginx)
docker-compose --profile production up -d

# Check status
docker-compose ps

# View logs
docker-compose logs -f
```

## Database Setup

The single Postgres instance uses schemas for isolation:
- `rust_service` - Rust backend tables
- `python_service` - Python services tables

The init script `postgres/init-schemas.sql` runs automatically on first start.

### Manual Schema Creation (if needed)
```bash
psql -h localhost -U app_user -d app_database
CREATE SCHEMA IF NOT EXISTS rust_service;
CREATE SCHEMA IF NOT EXISTS python_service;
SET search_path TO rust_service, python_service, public;
```

## Performance Tuning

### Postgres
Edit `postgres/postgres.conf` and restart:
```bash
docker-compose restart postgres
```

### Redis
Edit command in `docker-compose.yml`:
```yaml
command: redis-server --maxmemory 1gb --maxmemory-policy allkeys-lru --appendonly yes
```

### Rust Backend
Adjust connection pool in `rust-backend/src/main.rs`:
```rust
.max_connections(50)  // Increase for production
.min_connections(10)
```

## Monitoring Setup

### Grafana Dashboards
1. Login to Grafana (admin/admin)
2. Add Prometheus data source: `http://prometheus:9090`
3. Import pre-configured dashboards from `monitoring/grafana/dashboards/`

### Available Metrics
- Rust Backend: `http://localhost:8001/metrics`
- Python Services: `http://localhost:8000/metrics`
- Prometheus: `http://localhost:9090/metrics`

## Troubleshooting

### Port Conflicts
```bash
# Check what's using a port
lsof -i :5432
# Change ports in .env and docker-compose.yml
```

### Database Connection Issues
```bash
# Check Postgres logs
docker-compose logs postgres

# Test connection
psql -h localhost -U app_user -d app_database
```

### Redis Connection Issues
```bash
# Check Redis logs
docker-compose logs redis

# Test connection
redis-cli -h localhost -p 6379 ping
```

### Rust Build Errors
```bash
# Clean and rebuild
cd rust-backend
cargo clean
cargo build
```

### Python Dependencies
```bash
cd python-services
uv pip install -e .
```

## Cleanup

```bash
# Stop all containers
docker-compose down

# Remove volumes (DELETES DATA)
docker-compose down -v

# Full cleanup (including built images)
docker-compose down -v --rmi all

# Run cleanup script
./scripts/cleanup.sh
```

## Next Steps

- Explore the API endpoints (see [SERVICES.md](./SERVICES.md))
- Run load tests to measure performance
- Customize Grafana dashboards
- Add your own features to the services
