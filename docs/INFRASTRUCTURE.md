# Infrastructure Documentation

Single source of truth for recreating the development infrastructure from scratch.

## Table of Contents

1. [Quick Start](#quick-start)
2. [Architecture](#architecture)
3. [Services](#services)
   - [PostgreSQL 18](#postgresql-18-alpine)
   - [Redis 8](#redis-8-alpine)
   - [RustFS (S3-Compatible)](#rustfs-s3-compatible)
   - [Optional Tools](#optional-tools-profiles)
4. [Environment Variables](#environment-variables)
5. [Complete docker-compose.dev.yml](#complete-docker-composedevyml)
6. [Common Commands](#common-commands)
7. [Recreating from Scratch](#recreating-from-scratch)
8. [RustFS Usage](#rustfs-usage)
9. [Troubleshooting](#troubleshooting)
10. [Network Architecture](#network-architecture)
11. [Volumes](#volumes)
12. [Migration to Production](#migration-to-production)
13. [Updates](#updates)

## Quick Start

```bash
# Clone and start all infrastructure
git clone <repo>
cd bare-metal-demo

# Copy environment template
cp .env.example .env

# Start all services
docker compose -f docker-compose.dev.yml up -d

# Verify
docker compose -f docker-compose.dev.yml ps
```

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    bare-metal-network (172.20.0.0/16)         │
│                                                                 │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐   │
│  │ Postgres │  │  Redis   │  │  RustFS  │  │ Adminer  │   │
│  │   :5432  │  │  :6379  │  │:9000:9001│  │  :8080  │   │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘  └──────────┘   │
│       │               │               │                          │
│       ▼               ▼               ▼                          │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │              Rust Backend (:8001)                      │   │
│  │         (connects to all three services)               │   │
│  └─────────────────────────────────────────────────────────┘   │
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
| Container | `bare_metal_db` |
| Port | `5432` (configurable: `POSTGRES_PORT`) |
| Username | `app_user` (configurable: `POSTGRES_USER`) |
| Password | `app_pass` (configurable: `POSTGRES_PASSWORD`) |
| Database | `app_database` (configurable: `POSTGRES_DB`) |

**Connection string:** `postgres://app_user:app_pass@localhost:5432/app_database`

**Health check:** Uses `pg_isready` (10s interval, 5 retries)

**Data persistence:** Volume `postgres_data` → `/var/lib/postgresql/data`

### Redis 8 (Alpine)

In-memory cache and session store.

| Property | Value |
|----------|-------|
| Image | `redis:8-alpine` |
| Container | `bare_metal_redis` |
| Port | `6379` (configurable: `REDIS_PORT`) |
| Max Memory | `512mb` (configurable: `REDIS_MAX_MEMORY`) |
| Eviction Policy | `allkeys-lru` (configurable: `REDIS_MAXMEMORY_POLICY`) |

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
| Container | `bare_metal_rustfs` |
| API Port | `9000` (configurable: `RUSTFS_API_PORT`) |
| Console Port | `9001` (configurable: `RUSTFS_CONSOLE_PORT`) |
| Access Key | `devadmin` (configurable: `RUSTFS_ACCESS_KEY`) |
| Secret Key | `devadmin` (configurable: `RUSTFS_SECRET_KEY`) |
| Console | Enabled (web UI at `http://localhost:9001`) |

**Endpoint:** `http://localhost:9000`

**Console URL:** `http://localhost:9001` (login with access/secret keys)

**Health check:** `curl -f http://localhost:9000/minio/health/live`

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
docker compose -f docker-compose.dev.yml --profile tools up -d
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
POSTGRES_PASSWORD=app_pass
POSTGRES_DB=app_database
POSTGRES_PORT=5432
DATABASE_URL=postgres://app_user:app_pass@localhost:5432/app_database
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
RUSTFS_ACCESS_KEY=devadmin
RUSTFS_SECRET_KEY=devadmin
RUSTFS_CORS_ORIGINS=*
RUSTFS_ENDPOINT=http://localhost:9000
```

### Tool Ports (Optional)

```env
# Admin tools
ADMINER_PORT=8080
REDIS_COMMANDER_PORT=8081
```

## Complete docker-compose.dev.yml

This is the canonical reference. Always check this file for the latest configuration.

```yaml
version: '3.9'

networks:
  bare-metal-network:
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
    container_name: bare_metal_db
    restart: unless-stopped
    ports:
      - "${POSTGRES_PORT:-5432}:5432"
    environment:
      POSTGRES_USER: ${POSTGRES_USER:-app_user}
      POSTGRES_PASSWORD: ${POSTGRES_PASSWORD:-app_pass}
      POSTGRES_DB: ${POSTGRES_DB:-app_database}
      POSTGRES_INITDB_ARGS: "--encoding=UTF-8 --locale=C"
    volumes:
      - postgres_data:/var/lib/postgresql/data
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U ${POSTGRES_USER:-app_user} -d ${POSTGRES_DB:-app_database}"]
      interval: 10s
      timeout: 5s
      retries: 5
      start_period: 10s
    networks:
      - bare-metal-network
    logging:
      driver: "json-file"
      options:
        max-size: "10m"
        max-file: "3"

  redis:
    image: redis:8-alpine
    container_name: bare_metal_redis
    restart: unless-stopped
    ports:
      - "${REDIS_PORT:-6379}:6379"
    command: >
      redis-server
      --maxmemory ${REDIS_MAX_MEMORY:-512mb}
      --maxmemory-policy ${REDIS_MAXMEMORY_POLICY:-allkeys-lru}
      --appendonly ${REDIS_APPENDONLY:-yes}
      --save ${REDIS_SAVE:-900 1 300 10 60 10000}
    volumes:
      - redis_data:/data
    healthcheck:
      test: ["CMD", "redis-cli", "ping"]
      interval: 10s
      timeout: 3s
      retries: 5
      start_period: 5s
    networks:
      - bare-metal-network
    logging:
      driver: "json-file"
      options:
        max-size: "10m"
        max-file: "3"

  rustfs:
    image: rustfs/rustfs:latest
    container_name: bare_metal_rustfs
    restart: unless-stopped
    ports:
      - "${RUSTFS_API_PORT:-9000}:9000"
      - "${RUSTFS_CONSOLE_PORT:-9001}:9001"
    environment:
      RUSTFS_VOLUMES: /data
      RUSTFS_ADDRESS: 0.0.0.0:9000
      RUSTFS_CONSOLE_ADDRESS: 0.0.0.0:9001
      RUSTFS_CONSOLE_ENABLE: "true"
      RUSTFS_CORS_ALLOWED_ORIGINS: ${RUSTFS_CORS_ORIGINS:-*}
      RUSTFS_CONSOLE_CORS_ALLOWED_ORIGINS: ${RUSTFS_CORS_ORIGINS:-*}
      RUSTFS_ACCESS_KEY: ${RUSTFS_ACCESS_KEY:-devadmin}
      RUSTFS_SECRET_KEY: ${RUSTFS_SECRET_KEY:-devadmin}
      RUSTFS_BROWSER: "on"
    volumes:
      - rustfs_data:/data
    healthcheck:
      test: ["CMD-SHELL", "curl -f http://localhost:9000/minio/health/live || exit 1"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 15s
    networks:
      - bare-metal-network
    logging:
      driver: "json-file"
      options:
        max-size: "10m"
        max-file: "3"

  adminer:
    image: adminer:latest
    container_name: bare_metal_adminer
    restart: unless-stopped
    ports:
      - "${ADMINER_PORT:-8080}:8080"
    environment:
      ADMINER_DEFAULT_SERVER: postgres
    networks:
      - bare-metal-network
    profiles:
      - tools
    depends_on:
      postgres:
        condition: service_healthy

  redis-commander:
    image: rediscommander/redis-commander:latest
    container_name: bare_metal_redis_cmdr
    restart: unless-stopped
    ports:
      - "${REDIS_COMMANDER_PORT:-8081}:8081"
    environment:
      REDIS_HOST: redis
      REDIS_PORT: 6379
    networks:
      - bare-metal-network
    profiles:
      - tools
    depends_on:
      redis:
        condition: service_healthy
```

## Common Commands

### Start/Stop

```bash
# Start all core services (detached)
docker compose -f docker-compose.dev.yml up -d

# Start with optional tools
docker compose -f docker-compose.dev.yml --profile tools up -d

# Stop all services (keep data)
docker compose -f docker-compose.dev.yml stop

# Stop and remove containers (keep volumes)
docker compose -f docker-compose.dev.yml down

# Stop and remove everything (INCLUDING volumes - data lost!)
docker compose -f docker-compose.dev.yml down -v
```

### Monitoring

```bash
# Check service status
docker compose -f docker-compose.dev.yml ps

# View logs (all services)
docker compose -f docker-compose.dev.yml logs -f

# View logs (single service)
docker compose -f docker-compose.dev.yml logs -f postgres

# Check health status
docker inspect bare_metal_db --format='{{json .State.Health}}' | jq
```

### Shell Access

```bash
# Postgres shell
docker exec -it bare_metal_db psql -U app_user app_database

# Redis CLI
docker exec -it bare_metal_redis redis-cli

# RustFS shell
docker exec -it bare_metal_rustfs sh

# Check RustFS bucket list (requires mc client in container)
docker exec -it bare_metal_rustfs sh -c 'rustfs client ls'
```

### Database Operations

```bash
# Backup Postgres
docker exec bare_metal_db pg_dump -U app_user app_database > backup_$(date +%Y%m%d).sql

# Restore Postgres
docker exec -i bare_metal_db psql -U app_user app_database < backup_20260425.sql

# Redis backup (RDB file)
docker cp bare_metal_redis:/data/dump.rdb ./redis_backup.rdb
```

## Recreating from Scratch

To completely rebuild the infrastructure:

```bash
# 1. Stop and remove everything
docker compose -f docker-compose.dev.yml down -v

# 2. Remove any orphaned volumes
docker volume prune -f

# 3. Remove any orphaned networks
docker network prune -f

# 4. Recreate and start fresh
docker compose -f docker-compose.dev.yml up -d

# 5. Verify health
docker compose -f docker-compose.dev.yml ps

# 6. Check logs for any startup errors
docker compose -f docker-compose.dev.yml logs --tail=50
```

**What gets recreated:**
- 3 containers (postgres, redis, rustfs)
- 3 volumes (postgres_data, redis_data, rustfs_data) → **data is lost with `-v`
- 1 network (bare-metal-network)

**What persists:**
- `docker-compose.dev.yml` (configuration)
- `.env` (environment variables)
- Application code (outside Docker)

## RustFS Usage

### Creating a Bucket (S3-compatible)

```bash
# Using AWS CLI (configure with RustFS credentials)
aws --endpoint-url http://localhost:9000 s3 mb s3://my-bucket

# Using RustFS client (if available in container)
docker exec -it bare_metal_rustfs rustfs client mb /my-bucket
```

### Web Console

1. Open `http://localhost:9001`
2. Login with:
   - Access Key: `devadmin` (or `RUSTFS_ACCESS_KEY`)
   - Secret Key: `devadmin` (or `RUSTFS_SECRET_KEY`)
3. Create buckets, upload files, manage permissions

## Troubleshooting

### Port Conflicts

```bash
# Check what's using a port
lsof -i :5432  # Postgres
lsof -i :6379  # Redis
lsof -i :9000  # RustFS API

# Change ports in .env
POSTGRES_PORT=5433
REDIS_PORT=6380
RUSTFS_API_PORT=9002
```

### Service Won't Start

```bash
# Check logs for specific service
docker compose -f docker-compose.dev.yml logs postgres

# Check if ports are already in use
docker ps  # See all running containers

# Restart a specific service
docker compose -f docker-compose.dev.yml restart postgres
```

### Data Persistence Issues

```bash
# Check volume mount points
docker volume inspect bare_metal_demo_postgres_data

# See where data is stored on host
docker inspect bare_metal_db | jq '.[0].Mounts'

# Manually backup before destructive operations
docker cp bare_metal_db:/var/lib/postgresql/data ./postgres_backup
```

### Health Check Failures

```bash
# Manually test health checks
docker exec bare_metal_db pg_isready -U app_user -d app_database
docker exec bare_metal_redis redis-cli ping
docker exec bare_metal_rustfs curl -f http://localhost:9000/minio/health/live

# Check health status
docker inspect bare_metal_db --format='{{.State.Health.Status}}'
```

### RustFS Console Not Loading

```bash
# Verify RustFS is running
docker ps | grep rustfs

# Check RustFS logs
docker logs bare_metal_rustfs

# Ensure CORS is configured for your frontend
# Add to .env: RUSTFS_CORS_ORIGINS=http://localhost:4321
```

## Network Architecture

Services communicate over `bare-metal-network` (172.20.0.0/16):

| Service | Container Name | Internal IP | External Port |
|---------|----------------|--------------|---------------|
| Postgres | `bare_metal_db` | 172.20.0.10* | `5432` |
| Redis | `bare_metal_redis` | 172.20.0.11* | `6379` |
| RustFS | `bare_metal_rustfs` | 172.20.0.12* | `9000`, `9001` |

*IPs are dynamically assigned by Docker. Use container names for inter-service communication:
- Postgres: `postgres:5432`
- Redis: `redis:6379`
- RustFS: `rustfs:9000`

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

For production, consider:

1. **Secrets management:** Use Docker secrets or vault, not `.env`
2. **Resource limits:** Uncomment `deploy.resources` in compose file
3. **Backups:** Automated daily backups of all volumes
4. **Monitoring:** Add Prometheus + Grafana stack
5. **TLS:** Enable HTTPS for RustFS console
6. **Auth:** Change default credentials immediately

## Updates

```bash
# Pull latest images
docker compose -f docker-compose.dev.yml pull

# Recreate containers with new images
docker compose -f docker-compose.dev.yml up -d --force-recreate

# Clean up old images
docker image prune -f
```

## Next Steps

- See [ARCHITECTURE.md](./ARCHITECTURE.md) for system design
- See [SETUP.md](./SETUP.md) for installation guide
- See [SERVICES.md](./SERVICES.md) for service documentation
