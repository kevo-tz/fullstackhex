# Infrastructure Documentation

Canonical reference for recreating the development infrastructure from scratch.

## Table of Contents

1. [Quick Start](#quick-start)
2. [Architecture](#architecture)
3. [Services](#services)
   - [PostgreSQL 18](#postgresql-18-alpine)
   - [Redis 8](#redis-8-alpine)
   - [RustFS (S3-Compatible)](#rustfs-s3-compatible)
   - [Optional Tools](#optional-tools-profiles)
4. [Environment Variables](#environment-variables)
5. [Complete compose/dev.yml](#complete-compose-devyml)
6. [Common Commands](#common-commands)
7. [Recreating from Scratch](#recreating-from-scratch)
8. [RustFS Usage](#rustfs-usage)
9. [Troubleshooting](#troubleshooting)
10. [Network Architecture](#network-architecture)
11. [Volumes](#volumes)
12. [Migration to Production](#migration-to-production)
13. [Nginx Configuration](#nginx-configuration)
 14. [Monitoring Stack](#monitoring-stack)
15. [Compose Directory Layout](#compose-directory-layout)
16. [Updates](#updates)

## Quick Start

```bash
# Clone and start all infrastructure
git clone <repo>
cd fullstackhex

# Copy environment template
cp .env.example .env

# Start all services
docker compose -f compose/dev.yml up -d

# Verify
docker compose -f compose/dev.yml ps

# Optional: monitoring stack (Prometheus + Grafana)
docker compose -f compose/monitor.yml up -d
```

### Monitoring Overlay

The monitoring stack is defined in `compose/monitor.yml` and is designed to run alongside the main stack.

- Prometheus config: `monitoring/prometheus.yml`
- Grafana datasource provisioning: `monitoring/grafana/provisioning/datasources/prometheus.yml`
- Grafana dashboard provisioning: `monitoring/grafana/provisioning/dashboards/dashboards.yml`
- Starter dashboard: `monitoring/grafana/dashboards/overview.json`

Use the monitoring-specific environment values in `.env` or `.env.prod.example`:

```env
PROMETHEUS_PORT=9090
GRAFANA_PORT=3000
GRAFANA_ADMIN_USER=admin
GRAFANA_ADMIN_PASSWORD=CHANGE_ME
```

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│            fullstackhex-network (172.20.0.0/16)                 │
│                                                                 │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐                       │
│  │ Postgres │  │  Redis   │  │  RustFS  │                       │
│  │   :5432  │  │  :6379   │  │:9000:9001│                       │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘                       │
│       │             │             │                             │
│       ▼             ▼             ▼                             │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │              Rust Backend (:8001)                       │    │
│  │         (connects to all three services)                │    │
│  └─────────────────────────────────────────────────────────┘    │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘

Volumes (persistent data):
  - postgres_data  → /var/lib/postgresql/data
  - redis_data     → /data
  - rustfs_data    → /data
```

## Services

### PostgreSQL 18 (Alpine)

Primary relational database for the application.

| Property | Value |
|----------|-------|
| Image | `postgres:18-alpine` |
| Container | `fullstackhex_db` |
| Port | `5432` (configurable: **POSTGRES_PORT**) |
| Username | `app_user` (configurable: **POSTGRES_USER**) |
| Password | `CHANGE_ME` (set via **POSTGRES_PASSWORD** in `.env`) |
| Database | `app_database` (configurable: **POSTGRES_DB**) |

**Connection string:** `postgres://app_user:CHANGE_ME@localhost:5432/app_database`

**Health check:** Uses `pg_isready` (10s interval, 5 retries)

**Data persistence:** Volume `postgres_data` → `/var/lib/postgresql/data`

### Redis 8 (Alpine)

In-memory cache and session store.

| Property | Value |
|----------|-------|
| Image | `redis:8-alpine` |
| Container | `fullstackhex_redis` |
| Port | `6379` (configurable: **REDIS_PORT**) |
| Max Memory | `512mb` (configurable: **REDIS_MAX_MEMORY**) |
| Eviction Policy | `allkeys-lru` (configurable: **REDIS_MAXMEMORY_POLICY**) |

**Connection string:** `redis://localhost:6379`

**Health check:** Uses `redis-cli ping` (10s interval, 5 retries)

**Configuration:**
- `--appendonly yes` → Enables AOF persistence
- `--save 900 1 300 10 60 10000` → RDB snapshots

**Data persistence:** Volume `redis_data` → `/data`

### RustFS (S3-Compatible)

Open-source S3-compatible object storage server.

| Property | Value |
|----------|-------|
| Image | `rustfs/rustfs:latest` |
| Container | `fullstackhex_rustfs` |
| API Port | `9000` (configurable: **RUSTFS_API_PORT**) |
| Console Port | `9001` (configurable: **RUSTFS_CONSOLE_PORT**) |
| Access Key | `CHANGE_ME` (set via **RUSTFS_ACCESS_KEY** in `.env`) |
| Secret Key | `CHANGE_ME` (set via **RUSTFS_SECRET_KEY** in `.env`) |
| Console | Enabled (web UI at `http://localhost:9001`) |

**Endpoint:** `http://localhost:9000`

**Console URL:** `http://localhost:9001` (login with access/secret keys)

**Health check:** `curl -f http://localhost:9000/health`

**Data persistence:** Volume `rustfs_data` → `/data`

**Environment variables:**
- `RUSTFS_VOLUMES=/data` → Data directory
- `RUSTFS_ADDRESS=0.0.0.0:9000` → API bind address
- `RUSTFS_CONSOLE_ADDRESS=0.0.0.0:9001` → Console bind address
- `RUSTFS_CORS_ALLOWED_ORIGINS=*` → CORS for web clients
- `RUSTFS_BROWSER=on` → Enable web console

### Optional Tools (Profiles)

Enable with `--profile tools`:

**Adminer** (Database UI): `http://localhost:8080`
- Image: `adminer:latest`
- Profile: `tools`
- Depends on: healthy Postgres

**Redis Commander** (Redis UI): `http://localhost:8081`
- Image: `rediscommander/redis-commander:latest`
- Profile: `tools`
- Depends on: healthy Redis

```bash
# Start with optional tools
docker compose -f compose/dev.yml --profile tools up -d
```

## Environment Variables

Create `.env` from `.env.example`:

```bash
cp .env.example .env
```

### Database Configuration

```env
# PostgreSQL
POSTGRES_USER=app_user
POSTGRES_PASSWORD=CHANGE_ME
POSTGRES_DB=app_database
POSTGRES_PORT=5432
DATABASE_URL=postgres://app_user:CHANGE_ME@localhost:5432/app_database
```

### Cache Configuration

```env
# Redis
REDIS_PORT=6379
REDIS_MAX_MEMORY=512mb
REDIS_MAXMEMORY_POLICY=allkeys-lru
REDIS_APPENDONLY=yes
REDIS_SAVE=900 1 300 10 60 10000
REDIS_URL=redis://localhost:6379
```

### RustFS Configuration

```env
# RustFS (S3-compatible)
RUSTFS_API_PORT=9000
RUSTFS_CONSOLE_PORT=9001
RUSTFS_ACCESS_KEY=CHANGE_ME
RUSTFS_SECRET_KEY=CHANGE_ME
RUSTFS_CORS_ORIGINS=*
RUSTFS_ENDPOINT=http://localhost:9000
```

### Tool Ports (Optional)

```env
# Admin tools
ADMINER_PORT=8080
REDIS_COMMANDER_PORT=8081
```

## Complete compose/dev.yml

This is the canonical reference. Always check this file for the latest configuration.

```yaml
# Development Infrastructure for FullStackHex
# Usage: docker compose -f compose/dev.yml up -d
#
# Services:
#   - postgres:18-alpine  (port 5432) - Primary database
#   - redis:8-alpine      (port 6379) - Cache layer
#   - rustfs/rustfs:latest (ports 9000, 9001) - S3-compatible object storage
#
# Networks: fullstackhex-network (bridge)
# Volumes: postgres_data, redis_data, rustfs_data (persistent)

networks:
  fullstackhex-network:
    driver: bridge
    ipam:
      config:
        - subnet: 172.20.0.0/16

volumes:
  postgres_data:
    driver: local
  redis_data:
    driver: local
  rustfs_data:
    driver: local

services:
  postgres:
    image: postgres:18-alpine
    container_name: fullstackhex_db
    restart: unless-stopped
    ports:
      - "${POSTGRES_PORT:-5432}:5432"
    environment:
      POSTGRES_USER: ${POSTGRES_USER:-app_user}
      # POSTGRES_PASSWORD must be set in .env — no default is intentional
      POSTGRES_PASSWORD: ${POSTGRES_PASSWORD:?POSTGRES_PASSWORD must be set in .env}
      POSTGRES_DB: ${POSTGRES_DB:-app_database}
      # Tune for development (not production)
      POSTGRES_INITDB_ARGS: "--encoding=UTF-8 --locale=C"
    volumes:
      - postgres_data:/var/lib/postgresql/data
      # Optional: mount init scripts
      # - ./scripts/db/init:/docker-entrypoint-initdb.d
    healthcheck:
      test: [ "CMD-SHELL", "pg_isready -U ${POSTGRES_USER:-app_user} -d ${POSTGRES_DB:-app_database}" ]
      interval: 10s
      timeout: 5s
      retries: 5
      start_period: 10s
    networks:
      - fullstackhex-network
    logging:
      driver: "json-file"
      options:
        max-size: "10m"
        max-file: "3"

  redis:
    image: redis:8-alpine
    container_name: fullstackhex_redis
    restart: unless-stopped
    ports:
      - "${REDIS_PORT:-6379}:6379"
    command: >
      redis-server --maxmemory ${REDIS_MAX_MEMORY:-512mb} --maxmemory-policy ${REDIS_MAXMEMORY_POLICY:-allkeys-lru} --appendonly ${REDIS_APPENDONLY:-yes} --save ${REDIS_SAVE:-900 1 300 10 60 10000}
    volumes:
      - redis_data:/data
    healthcheck:
      test: [ "CMD", "redis-cli", "ping" ]
      interval: 10s
      timeout: 3s
      retries: 5
      start_period: 5s
    networks:
      - fullstackhex-network
    logging:
      driver: "json-file"
      options:
        max-size: "10m"
        max-file: "3"

  rustfs:
    image: rustfs/rustfs:latest
    container_name: fullstackhex_rustfs
    restart: unless-stopped
    ports:
      - "${RUSTFS_API_PORT:-9000}:9000"
      - "${RUSTFS_CONSOLE_PORT:-9001}:9001"
    environment:
      # RustFS configuration
      RUSTFS_VOLUMES: /data
      RUSTFS_ADDRESS: 0.0.0.0:9000
      RUSTFS_CONSOLE_ADDRESS: 0.0.0.0:9001
      RUSTFS_CONSOLE_ENABLE: "true"
      RUSTFS_CORS_ALLOWED_ORIGINS: ${RUSTFS_CORS_ORIGINS:-*}
      RUSTFS_CONSOLE_CORS_ALLOWED_ORIGINS: ${RUSTFS_CORS_ORIGINS:-*}
      # Credentials must be set in .env — no defaults are intentional
      RUSTFS_ACCESS_KEY: ${RUSTFS_ACCESS_KEY:?RUSTFS_ACCESS_KEY must be set in .env}
      RUSTFS_SECRET_KEY: ${RUSTFS_SECRET_KEY:?RUSTFS_SECRET_KEY must be set in .env}
      # Enable browser access
      RUSTFS_BROWSER: "on"
    volumes:
      - rustfs_data:/data
    healthcheck:
      test: [ "CMD", "sh", "-c", "curl -f http://127.0.0.1:9000/health && curl -f http://127.0.0.1:9001/rustfs/console/health" ]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 15s
    networks:
      - fullstackhex-network
    logging:
      driver: "json-file"
      options:
        max-size: "10m"
        max-file: "3"

  adminer:
    image: adminer:latest
    container_name: fullstackhex_adminer
    restart: unless-stopped
    ports:
      - "${ADMINER_PORT:-8080}:8080"
    depends_on:
      postgres:
        condition: service_healthy
    networks:
      - fullstackhex-network
    profiles:
      - tools

  redis-commander:
    image: rediscommander/redis-commander:latest
    container_name: fullstackhex_redis_commander
    restart: unless-stopped
    ports:
      - "${REDIS_COMMANDER_PORT:-8081}:8081"
    environment:
      REDIS_HOSTS: local:fullstackhex_redis:6379
    depends_on:
      redis:
        condition: service_healthy
    networks:
      - fullstackhex-network
    profiles:
      - tools

```

## Common Commands

### Start/Stop

```bash
# Start all core services (detached)
docker compose -f compose/dev.yml up -d

# Start with optional tools
docker compose -f compose/dev.yml --profile tools up -d

# Stop all services (keep data)
docker compose -f compose/dev.yml stop

# Stop and remove containers (keep volumes)
docker compose -f compose/dev.yml down

# Stop and remove everything (INCLUDING volumes - data lost!)
docker compose -f compose/dev.yml down -v
```

### Monitoring

```bash
# Check service status
docker compose -f compose/dev.yml ps

# View logs (all services)
docker compose -f compose/dev.yml logs -f

# View logs (single service)
docker compose -f compose/dev.yml logs -f postgres

# Check health status
docker inspect fullstackhex_db --format='{{json .State.Health}}' | jq
```

### Shell Access

```bash
# Postgres shell
docker exec -it fullstackhex_db psql -U app_user app_database

# Redis CLI
docker exec -it fullstackhex_redis redis-cli

# RustFS shell
docker exec -it fullstackhex_rustfs sh

# Check RustFS bucket list (requires mc client in container)
docker exec -it fullstackhex_rustfs sh -c 'rustfs client ls'
```

### Database Operations

```bash
# Backup Postgres
docker exec fullstackhex_db pg_dump -U app_user app_database > backup_$(date +%Y%m%d).sql

# Restore Postgres
docker exec -i fullstackhex_db psql -U app_user app_database < backup_20260425.sql

# Redis backup (RDB file)
docker cp fullstackhex_redis:/data/dump.rdb ./redis_backup.rdb
```

## Recreating from Scratch

To completely rebuild the infrastructure:

```bash
# 1. Stop and remove everything
docker compose -f compose/dev.yml down -v

# 2. Remove any orphaned volumes
docker volume prune -f

# 3. Remove any orphaned networks
docker network prune -f

# 4. Recreate and start fresh
docker compose -f compose/dev.yml up -d

# 5. Verify health
docker compose -f compose/dev.yml ps

# 6. Check logs for any startup errors
docker compose -f compose/dev.yml logs --tail=50
```

**What gets recreated:**
- 3 containers (postgres, redis, rustfs)
- 3 volumes (postgres_data, redis_data, rustfs_data) → **data is lost with `-v`**
- 1 network (fullstackhex-network)

**What persists:**
- `compose/dev.yml` (configuration)
- `.env` (environment variables)
- Application code (outside Docker)

## RustFS Usage

### Creating a Bucket (S3-compatible)

```bash
# Using AWS CLI (configure with RustFS credentials)
aws --endpoint-url http://localhost:9000 s3 mb s3://my-bucket

# Using RustFS client (if available in container)
docker exec -it fullstackhex_rustfs rustfs client mb /my-bucket
```

### Web Console

1. Open `http://localhost:9001`
2. Login with:
   - Access Key: value of **RUSTFS_ACCESS_KEY** from your `.env`
   - Secret Key: value of **RUSTFS_SECRET_KEY** from your `.env`
3. Create buckets, upload files, manage permissions

## Troubleshooting

### Port Conflicts

```bash
# Check what's using a port
lsof -i :5432  # Postgres
lsof -i :6379  # Redis
lsof -i :9000  # RustFS API

# Check for stuck Docker containers holding ports
docker ps  # List running containers
docker rm -f <container_name>  # Force-remove stuck container

# Change ports in .env
POSTGRES_PORT=5433
REDIS_PORT=6380
RUSTFS_API_PORT=9002
```

### Service Won't Start

```bash
# Check logs for specific service
docker compose -f compose/dev.yml logs postgres

# Check if ports are already in use
docker ps  # See all running containers

# Restart a specific service
docker compose -f compose/dev.yml restart postgres
```

### Data Persistence Issues

```bash
# Check volume mount points
docker volume inspect fullstackhex_postgres_data

# See where data is stored on host
docker inspect fullstackhex_db | jq '.[0].Mounts'

# Manually backup before destructive operations
docker cp fullstackhex_db:/var/lib/postgresql/data ./postgres_backup
```

### Health Check Failures

```bash
# Manually test health checks
docker exec fullstackhex_db pg_isready -U app_user -d app_database
docker exec fullstackhex_redis redis-cli ping
docker exec fullstackhex_rustfs curl -f http://localhost:9000/health

# Check health status
docker inspect fullstackhex_db --format='{{.State.Health.Status}}'
```

### RustFS Console Not Loading

```bash
# Verify RustFS is running
docker ps | grep rustfs

# Check RustFS logs
docker logs fullstackhex_rustfs

# Ensure CORS is configured for your frontend
# Add to .env: RUSTFS_CORS_ORIGINS=http://localhost:4321
```

## Network Architecture

Services communicate over `fullstackhex-network` (172.20.0.0/16):

| Service | Container Name | Inter-service DNS | External Port |
|---------|----------------|-------------------|---------------|
| Postgres | `fullstackhex_db` | `postgres:5432` | `5432` |
| Redis | `fullstackhex_redis` | `redis:6379` | `6379` |
| RustFS | `fullstackhex_rustfs` | `rustfs:9000` | `9000`, `9001` |

IPs are dynamically assigned by Docker — use container service names (column 3) for inter-service communication, not IP addresses.

## Volumes

Persistent data stored in Docker volumes:

```bash
# List volumes
docker volume ls | grep bare

# Inspect volume
docker volume inspect <volume_name>

# Backup volume data
docker run --rm -v postgres_data:/data -v $(pwd):/backup alpine tar czf /backup/postgres.tar.gz /data

# Restore volume data
docker run --rm -v postgres_data:/data -v $(pwd):/backup alpine tar xzf /backup/postgres.tar.gz -C /
```

## Migration to Production

Use `compose/prod.yml` to run all services as Docker containers with no external port exposure (except Nginx).

### Production Services

| Service | Image | Internal port | Notes |
|---------|-------|---------------|-------|
| nginx | `nginx:alpine` | 80 / 443 | TLS termination, reverse proxy |
| backend | `Dockerfile.rust` | 8001 | Depends on postgres, redis, python-sidecar |
| python-sidecar | `Dockerfile.python` | Unix socket | Shares `sidecar_socket` volume with backend |
| frontend | `Dockerfile.frontend` | 4321 | Astro SSR node adapter |
| postgres | `postgres:18-alpine` | 5432 | Internal only (no host binding) |
| redis | `redis:8-alpine` | 6379 | Internal only |
| rustfs | `rustfs/rustfs:latest` | 9000 / 9001 | Internal only |

### Unix Socket in Production

In production, the sidecar socket is shared via a Docker named volume (`sidecar_socket` → `/tmp/sidecar/`):

```yaml
volumes:
  sidecar_socket:
    driver: local
```

Both `backend` and `python-sidecar` mount this volume at `/tmp/sidecar`. The socket path becomes `/tmp/sidecar/python-sidecar.sock`. Set in `.env`:

```env
PYTHON_SIDECAR_SOCKET=/tmp/sidecar/python-sidecar.sock
```

### TLS Certificates

Place certificates in `nginx/certs/` before starting:

```bash
nginx/certs/fullchain.pem
nginx/certs/privkey.pem
```

### Start Production Stack

```bash
cp .env.example .env
# Edit .env — replace ALL CHANGE_ME values and set PYTHON_SIDECAR_SOCKET
docker compose -f compose/prod.yml up -d
```

### Deploy to a VPS

The `make deploy` target pushes the stack to a remote server via SSH + rsync.

**Prerequisites:**
1. SSH key loaded in `ssh-agent` (or set `DEPLOY_SSH_KEY` in `.env`)
2. `.env` contains:
   ```env
   DEPLOY_HOST=your-vps.example.com
   DEPLOY_USER=ubuntu
   DEPLOY_PATH=/opt/fullstackhex
   ```
3. Docker + Docker Compose installed on the VPS

**Deploy:**
```bash
make deploy
```

This runs:
1. `rsync` compose files, nginx config, and `.env` to the VPS
2. `ssh` to run `docker compose -f compose/prod.yml up -d --wait`
3. `make deploy-check` polls `https://$DEPLOY_HOST/health` until OK

**Restart (pull latest images):**
```bash
make prod-restart
```

**PostgreSQL backups:**
```bash
# One-liner for cron (runs on the VPS)
ssh $DEPLOY_USER@$DEPLOY_HOST "docker exec fullstackhex_db pg_dump -U $POSTGRES_USER $POSTGRES_DB" > backup_$(date +%Y%m%d).sql
```

---

## Nginx Configuration

Two config files in `nginx/`:

### `nginx/nginx.conf` — Production reverse proxy

Handles TLS termination and routing:

| Route | Upstream |
|-------|----------|
| `/` | `frontend:4321` (Astro SSR) |
| `/api/` | `backend:8001` (Axum) |

Key features:
- HTTP → HTTPS redirect (port 80 → 443)
- TLS 1.2 / 1.3 only with strong cipher suite
- HSTS, X-Content-Type-Options, X-Frame-Options headers
- Gzip compression for text, CSS, JS, JSON
- OCSP stapling

### `nginx/static.conf` — Optional static file serving

Minimal config for serving an Astro **static** build (no SSR) at port 4321. Not used when running Astro in SSR mode with the Node adapter.

---

## Monitoring Stack

The monitoring stack is in `compose/monitor.yml` and joins the existing `fullstackhex-network`. Run it alongside either dev or prod:

```bash
docker compose -f compose/monitor.yml up -d
```

### Monitoring Services

| Service | Image | Port | Purpose |
|---------|-------|------|---------|
| prometheus | `prom/prometheus:v3.3.1` | `9090` | Metrics scraping + storage |
| grafana | `grafana/grafana:11.2.0` | `3000` | Dashboards |

### Configuration Files

| File | Purpose |
|------|---------|
| `monitoring/prometheus.yml` | Scrape targets |
| `monitoring/grafana/provisioning/datasources/prometheus.yml` | Auto-wire Prometheus as Grafana datasource |
| `monitoring/grafana/provisioning/dashboards/dashboards.yml` | Dashboard provisioning path |
| `monitoring/grafana/dashboards/overview.json` | Starter overview dashboard |

### Monitoring `.env` Variables

```env
PROMETHEUS_PORT=9090
GRAFANA_PORT=3000
GRAFANA_ADMIN_USER=admin
GRAFANA_ADMIN_PASSWORD=CHANGE_ME
GRAFANA_DOMAIN=localhost
```

> **Note:** **GRAFANA_ADMIN_PASSWORD** must be set — `compose/monitor.yml` uses `:?` syntax and will fail at startup if missing.

---

---

## Compose Directory Layout

The `compose/` directory contains Dockerfiles and configuration files used by the Docker Compose files (`compose/dev.yml`, `compose/prod.yml`, `compose/monitor.yml`).

### Directory Structure

```
compose/
├── Dockerfile.rust              # Multi-stage Rust backend build
├── Dockerfile.python           # Multi-stage Python sidecar build
├── Dockerfile.frontend         # Multi-stage Astro frontend build (with nginx)
├── nginx/
│   ├── nginx.conf             # Nginx reverse proxy configuration
│   └── certs/                # TLS certificates (gitignored)
│       ├── fullchain.pem      # Full certificate chain (gitignored)
│       ├── privkey.pem        # Private key (gitignored)
│       └── README.md          # Instructions for obtaining certificates
├── prometheus.yml            # Prometheus scrape configuration
└── grafana/
    ├── provisioning/
    │   ├── datasources/      # Grafana datasource definitions
    │   │   └── prometheus.yml
    │   └── dashboards/       # Dashboard discovery config
    │       └── dashboards.yml
    └── dashboards/            # Pre-built dashboards
        └── overview.json      # Starter dashboard (p99, error rate, RPS)
```

### Dockerfiles

All production Dockerfiles are stored in `compose/` and use the **repository root** as build context:

| Dockerfile | Purpose | Build Context | Description |
|-----------|---------|--------------|-------------|
| `compose/Dockerfile.rust` | Rust backend | `..` (repo root) | Multi-stage: builds Rust app, minimal runtime |
| `compose/Dockerfile.python` | Python sidecar | `..` (repo root) | Multi-stage: installs deps, minimal Python runtime |
| `compose/Dockerfile.frontend` | Astro frontend | `..` (repo root) | Multi-stage: builds Astro site, serves with nginx |

**Why repo root as context?** Dockerfiles need access to source files in `backend/`, `python-sidecar/`, `frontend/`. Using `..` (parent directory) as context allows Dockerfiles in `compose/` to copy from the repo root.

Example from `compose/prod.yml`:
```yaml
backend:
  build:
    context: ..          # Repository root
    dockerfile: compose/Dockerfile.rust
```

### Nginx Configuration

`compose/nginx/nginx.conf` is mounted to the nginx container at `/etc/nginx/nginx.conf`.

**Key features:**
- Reverse proxy for Rust backend (`/api/` routes) and Astro frontend (all other routes)
- TLS termination at ports 80/443
- Health check endpoint proxy
- Gzip compression enabled

**TLS Certificates:**
- Place `fullchain.pem` and `privkey.pem` in `compose/nginx/certs/`
- See `compose/nginx/certs/README.md` for instructions
- **Never commit private keys to version control**

### Monitoring Configuration

| File | Purpose |
|------|---------|
| `compose/prometheus.yml` | Defines scrape targets (Rust backend, node-exporter) |
| `compose/grafana/provisioning/datasources/prometheus.yml` | Auto-provisions Prometheus as Grafana datasource |
| `compose/grafana/provisioning/dashboards/dashboards.yml` | Tells Grafana where to find dashboards |
| `compose/grafana/dashboards/overview.json` | Pre-built dashboard with key metrics |

---

## Updates

```bash
# Pull latest images
docker compose -f compose/dev.yml pull

# Recreate containers with new images
docker compose -f compose/dev.yml up -d --force-recreate

# Clean up old images
docker image prune -f
```

## Related Docs

- [Previous: SERVICES.md](./SERVICES.md) - Service details and communication
- [Next: INITIALIZATION.md](./INITIALIZATION.md) - Template-ready setup script
- [All Docs](./INDEX.md) - Full documentation index
