# System Architecture Overview

## High-Level System Design

The Bare Metal Demo is a high-performance distributed system with Rust, Python, and Astro.js (Bun SSR) components, featuring a unified infrastructure with monitoring.

### System Overview

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    Nginx (HTTP/2, Brotli/Gzip)                       │
│                         Ports 80/443                                  │
└──────────────┬──────────────────────────┬─────────────────────────┘
               │                          │
     ┌─────────▼──────────┐    ┌─────────▼──────────┐
      │   Astro Frontend    │    │   Rust Backend     │
      │ Port 4321 (Bun SSR)│    │  Port 8001 (Axum)  │
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
│              Monitoring Stack                                       │
│  Prometheus (9090) + Grafana (3000) + Metrics Endpoints         │
└─────────────────────────────────────────────────────────────────────┘
```

## Data Flow

### Request Flow
1. **Client Request** → Nginx (HTTP/2, compression)
2. **Static/SSR Content** → Astro Frontend (Bun)
3. **API Requests** → Rust Backend (Axum) or Python Services (FastAPI)
4. **Data Layer** → Postgres (single instance, multiple schemas) + Redis (cache)
5. **Object Storage** → RustFS (S3-compatible)

### Service Communication
- Frontend ↔ Rust Backend (HTTP API)
- Rust Backend ↔ Python Services (HTTP API, optional)
- All services ↔ Postgres/Redis (async connections)

## Technology Stack

### Astro Frontend (Bun SSR)
- **Framework**: Astro.js 4.x
- **Runtime**: Bun (SSR mode)
- **Output**: Standalone Node.js-compatible bundle
- **Performance**: HTTP/2, HMR in dev, optimized builds

### Rust Backend
- **Framework**: Axum 0.8 (HTTP/2 support)
- **Runtime**: Tokio (async)
- **Database**: PostgreSQL with sqlx (compile-time checks)
- **Cache**: Redis (async, connection manager)
- **Middleware**: Compression, CORS, Tracing, Rate Limiting
- **Metrics**: Prometheus endpoint (/metrics)
- **Performance**: 50 max DB connections, 10 min connections

### Python Services
- **Framework**: FastAPI (modern, fast)
- **Server**: Uvicorn (with uvloop for performance)
- **Database**: PostgreSQL with psycopg (async pool)
- **Cache**: Redis (asyncio)
- **Metrics**: Prometheus FastAPI Instrumentator
- **Performance**: uvloop, httptools, async connection pooling

### Data Layer
- **PostgreSQL 18**: Single instance, multiple schemas (`rust_service`, `python_service`)
  - Resource limits: 2 CPU, 2GB RAM
  - Tuning: shared_buffers=512MB, effective_cache_size=1536MB
- **Redis 8**: LRU eviction, 512MB maxmemory, AOF persistence
  - Resource limits: 1 CPU, 1GB RAM
- **RustFS**: S3-compatible object storage, MinIO-compatible API

### Infrastructure
- **Reverse Proxy**: Nginx (HTTP/2, Brotli/Gzip compression)
- **Monitoring**: Prometheus + Grafana
- **Orchestration**: Docker Compose with resource limits
- **All services**: Healthchecks, restart policies, graceful shutdown

## Performance Features

✅ **Async Everywhere**: Tokio (Rust), asyncio (Python), async/await (Redis)  
✅ **Connection Pooling**: Postgres (50 max), Redis (manager)  
✅ **HTTP/2**: Enabled via Axum + Nginx  
✅ **Compression**: Brotli/Gzip (Nginx + Tower middleware)  
✅ **Caching**: Multi-layer (Redis + HTTP cache headers)  
✅ **Rate Limiting**: Tower governor (Rust backend)  
✅ **Observability**: Prometheus metrics + OpenTelemetry tracing  
✅ **Resource Management**: Docker container limits (CPU/memory)  
✅ **Persistence**: Redis AOF, Postgres WAL  

## Port Mappings

| External Port | Service              | Internal Port |
|---------------|----------------------|---------------|
| 80/443        | Nginx                | 80/443        |
| 4321          | Astro Frontend       | 4321          |
| 8001          | Rust Backend         | 8001          |
| 8000          | Python Services      | 8000          |
| 5432          | PostgreSQL           | 5432          |
| 6379          | Redis                | 6379          |
| 9000          | RustFS (API)         | 9000          |
| 9090          | Prometheus           | 9090          |
| 3000          | Grafana              | 3000          |

## Scaling Considerations

### Horizontal Scaling
- Frontend: Multiple replicas behind Nginx load balancer
- Backend: Multiple Rust instances with Redis for session storage
- Services: Multiple Python instances with connection pooling
- Database: Read replicas + connection pooling
- Redis: Cluster mode for high availability

### Vertical Scaling
- Increase container resource limits in docker-compose.yml
- Adjust Postgres shared_buffers, work_mem
- Increase connection pool sizes
- Enable Redis Cluster for higher throughput

## Security (Production)

- Enable JWT/OAuth2 authentication
- Restrict CORS to specific domains
- Use strong database credentials
- Enable SSL/TLS for all connections
- Implement rate limiting
- Use secrets management (not .env files)
- Regular security updates

## Next Steps

- See [SETUP.md](./SETUP.md) for installation instructions
- See [SERVICES.md](./SERVICES.md) for API documentation
- Review individual services
