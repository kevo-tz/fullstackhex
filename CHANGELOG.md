# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [0.1.0] - 2026-04-26

### Added
- Initial open source release of FullStackHex
- Rust backend (Axum + Tokio) serving HTTP API on port 8001
- Python sidecar (FastAPI + uvicorn) communicating via Unix domain socket
- Astro + Bun frontend on port 4321
- Docker Compose development stack: PostgreSQL 18, Redis 8, RustFS (S3-compatible)
- Multi-stage Dockerfiles for Rust backend, Python sidecar, and Astro frontend
- Production Docker Compose with resource limits, no optional tooling, Nginx service
- Nginx reverse proxy with TLS termination, security headers, and HTTP→HTTPS redirect
- GitHub Actions CI pipeline: `cargo fmt`/`clippy`/`test`, `ruff`/`pytest`, `bun lint`/`bun test`/`bun build`
- `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, `SECURITY.md`, issue templates, and PR template
- `.env.example` with all required environment variables documented

[0.1.0]: https://github.com/cevor/fullstackhex/releases/tag/v0.1.0
