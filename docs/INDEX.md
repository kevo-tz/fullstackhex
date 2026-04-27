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

4. **[INFRASTRUCTURE.md](./INFRASTRUCTURE.md)** - Docker setup and config
   - PostgreSQL 18, Redis 8, RustFS (S3-compatible)
   - Complete `compose/dev.yml` reference
   - Environment variables and volumes
   - Common commands and troubleshooting

5. **[INITIALIZATION.md](./INITIALIZATION.md)** - Template-ready setup script
   - Portable `scripts/install.sh` for new projects
   - Verification steps
   - Troubleshooting

6. **[performance-budget.md](./performance-budget.md)** - Performance targets and CI gates

## Quick Reference

| Document | Purpose | Time to Read |
|----------|---------|--------------|
| SETUP.md | Get running in 5 minutes | 3 min |
| ARCHITECTURE.md | Understand the stack | 2 min |
| SERVICES.md | Service API details | 4 min |
| INFRASTRUCTURE.md | Docker/infra reference | 5 min |
| INITIALIZATION.md | Template for new projects | 3 min |
| performance-budget.md | Performance targets and CI gates | 2 min |

## Related Docs

- [SETUP.md](./SETUP.md) - Start here for first-time setup
