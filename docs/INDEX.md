# Docs Index

Start here for the FullStackHex documentation. Documents are listed in recommended reading order for new users.

## Table of Contents

1. [User Journey](#user-journey)
2. [Quick Reference](#quick-reference)
3. [Related Docs](#related-docs)

## User Journey

1. **[SETUP.md](./SETUP.md)** - One-command init and tool install
   - Install Rust (edition 2024), Bun (latest), uv (latest)
   - Create Rust workspace structure
   - Start infrastructure and verify

2. **[ARCHITECTURE.md](./ARCHITECTURE.md)** - System design overview
   - Rust backend with Python sidecar architecture
   - Technology stack (latest versions)
   - IPC via Unix domain socket
   - Port mappings and data flow

3. **[SERVICES.md](./SERVICES.md)** - Service details and communication
    - Rust workspace crates and API endpoints
    - Python sidecar (FastAPI + Unix socket)
   - Frontend service implementation (Astro + Bun + Tailwind)
    - Health checks

4. **[AUTH.md](./AUTH.md)** - Authentication configuration
    - JWT access and refresh tokens
    - OAuth2 login (Google, GitHub)
    - Brute-force protection and CSRF
    - Python sidecar HMAC trust

5. **[STORAGE.md](./STORAGE.md)** - S3-compatible object storage
    - Presigned URLs, streaming, multipart upload
    - RustFS for local development

6. **[REDIS.md](./REDIS.md)** - Redis application layer
    - Caching, rate limiting, session store
    - Key namespaces and atomic refresh rotation

7. **[DEPLOY.md](./DEPLOY.md)** - Production deployment
    - Blue-green and canary deploy
    - Rollback and health verification

8. **[EXAMPLES.md](./EXAMPLES.md)** - Copy-paste patterns
    - Extending routes, pages, tests, and CI

4. **[INFRASTRUCTURE.md](./INFRASTRUCTURE.md)** - Docker setup and config
   - PostgreSQL 18, Redis 8, RustFS (S3-compatible)
   - Complete `compose/dev.yml` reference
   - Environment variables and volumes
   - Common commands and troubleshooting

5. **[INITIALIZATION.md](./INITIALIZATION.md)** - Template-ready setup script
   - Portable bootstrap script template for new projects
   - Verification steps
   - Troubleshooting

6. **[performance-budget.md](./performance-budget.md)** - Performance targets and CI gates

## Quick Reference

| Document | Purpose | Time to Read |
|----------|---------|--------------|
| SETUP.md | Get running in 5 minutes | 3 min |
| ARCHITECTURE.md | Understand the stack | 2 min |
| SERVICES.md | Service API details | 4 min |
| AUTH.md | JWT, OAuth, session config | 3 min |
| STORAGE.md | S3-compatible object storage | 3 min |
| REDIS.md | Redis caching and rate limiting | 3 min |
| DEPLOY.md | Blue-green, canary, rollback | 3 min |
| EXAMPLES.md | Copy-paste extension patterns | 4 min |
| INFRASTRUCTURE.md | Docker/infra reference | 5 min |
| MONITORING.md | Prometheus + Grafana setup | 4 min |
| CI.md | CI/CD pipeline reference | 3 min |
| INITIALIZATION.md | Template for new projects | 3 min |
| performance-budget.md | Performance targets and CI gates | 2 min |
| logging-conventions.md | Structured log format schema | 2 min |

## Related Docs

- [Previous: performance-budget.md](./performance-budget.md) - Performance targets and CI gates
- [MONITORING.md](./MONITORING.md) - Prometheus + Grafana setup and dashboards
- [CI.md](./CI.md) - CI/CD pipeline and debugging
- [logging-conventions.md](./logging-conventions.md) - Structured log format across languages
- [All Docs](./INDEX.md) - Full documentation index
