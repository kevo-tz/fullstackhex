# FullStackHex

A production-ready full-stack template combining a Rust/Axum backend, Python sidecar (FastAPI over Unix socket IPC), Astro frontend, and six pre-wired infrastructure components — all using the latest available tooling.

## Stack at a Glance

| Layer | Technology |
|-------|-----------|
| Frontend | Astro + Bun + Tailwind CSS |
| Backend | Rust (Axum, edition 2024) |
| Python sidecar | FastAPI + uv, connected via Unix domain socket |
| Cache | Redis 8 |
| Database | PostgreSQL 18 |
| Object storage | RustFS (S3-compatible) |

## Key Features

- **Rust + Python sidecar** — Rust backend spawns a Python FastAPI process and communicates over a Unix domain socket for low-latency IPC without network overhead.
- **Latest tooling** — Rust stable (edition 2024), Bun (latest), uv (latest), Astro v6.
- **One-command init** — `./scripts/install.sh` installs all tools, scaffolds the Rust workspace, and creates the Astro frontend.
- **Dev infrastructure via Docker Compose** — PostgreSQL 18, Redis 8, and RustFS spin up with a single command; optional Adminer and Redis Commander behind a `tools` profile.
- **Monitoring stack overlay** — `compose/monitor.yml` adds Prometheus + Grafana with provisioning and starter dashboard.
- **Generated test suites** — initialization scaffolds Rust/Python/Frontend unit, integration, and smoke tests by default.
- **Security automation** — local `detect-secrets` pre-commit checks plus CI `gitleaks` scanning.
- **Dependency automation** — Dependabot updates for Rust, Python, frontend, and GitHub Actions.
- **MIT licensed** — permissive license, use freely as a project starter.

## Quick Start

```bash
# 1. Clone
git clone https://github.com/kevo-tz/fullstackhex.git
cd fullstackhex

# 2. Configure secrets
cp .env.example .env
# Edit .env — replace every CHANGE_ME value before proceeding

# 3. Install tools and scaffold project
./scripts/install.sh

# 4. Start infrastructure
docker compose -f compose/dev.yml up -d

# 4b. Optional monitoring overlay
docker compose -f compose/monitor.yml up -d

# 5. Run backend (spawns Python sidecar automatically)
cd backend && cargo run --workspace

# 6. Run frontend
cd frontend && bun run dev
```

Prerequisites: Python 3.14+, Docker, Docker Compose. The install script handles Rust, Bun, and uv.

For production template setup, use `.env.prod.example`.

## Documentation

| Doc | Purpose |
|-----|---------|
| [docs/SETUP.md](docs/SETUP.md) | One-command init and tool install |
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | System design, IPC, port mappings |
| [docs/SERVICES.md](docs/SERVICES.md) | Crate layout, API endpoints, health checks |
| [docs/INFRASTRUCTURE.md](docs/INFRASTRUCTURE.md) | Docker Compose reference, volumes, commands |
| [docs/INITIALIZATION.md](docs/INITIALIZATION.md) | Portable template for new projects |

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for branch naming, PR process, and code style requirements.

## Security

To report a vulnerability, follow the policy in [.github/SECURITY.md](.github/SECURITY.md). Do not open a public issue.

## Quality Automation

- `CI` runs Rust/Python/frontend checks, generated template smoke tests, and security scans.
- `Dependabot` configuration lives at `.github/dependabot.yml`.
- Local secret scanning is configured via `.pre-commit-config.yaml` and `.github/.secrets.baseline`.

## License

[MIT](LICENSE) © 2026 FullStackHex Contributors
