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

- **Rust + Python sidecar** — Rust backend connects to a running Python FastAPI sidecar via Unix domain socket for low-latency IPC without network overhead.
- **Latest tooling** — Rust stable (edition 2024), Bun (latest), uv (latest), Astro v6.
- **Ships complete** — every source file, config, and test is committed. Clone and run — no scaffolding step.
- **`make setup`** — installs Rust, Bun, uv and creates `.env` from `.env.example`. That's all first-time setup requires.
- **Dev infrastructure via Docker Compose** — PostgreSQL 18, Redis 8, and RustFS spin up with a single command; optional Adminer and Redis Commander behind a `tools` profile.
- **Monitoring stack overlay** — `compose/monitor.yml` adds Prometheus + Grafana with 5 auto-provisioned dashboards (API, DB, Python, Infrastructure, SLOs).
- **Metrics out of the box** — Rust backend exposes `/metrics` with request counters and latency histograms; Python sidecar metrics proxied via `/metrics/python`.
- **Production deployable** — `make deploy` pushes to a VPS via SSH+rsync; nginx + TLS + certbot auto-renewal included.
- **Full test suites committed** — Rust/Python/Frontend unit, integration, and smoke tests ship in the repo.
- **Security automation** — local `detect-secrets` pre-commit checks plus CI `gitleaks` scanning.
- **Dependency automation** — Dependabot updates for Rust, Python, frontend, and GitHub Actions.
- **MIT licensed** — permissive license, use freely as a project starter.

## Quick Start

```bash
# 1. Clone
git clone https://github.com/kevo-tz/fullstackhex.git
cd fullstackhex

# 2. Install tools + create .env
make setup

# 3. Start everything (infra + Python sidecar + Rust backend + frontend)
make dev
```

Dashboard at http://localhost:4321 — three green dots means everything is healthy.

For infra-only (run backend/frontend manually): `make up`.

`make dev` runs everything in the foreground — press Ctrl+C to stop all services.
If you need services to survive terminal closure, start them individually:
```bash
make up                              # Docker services only
cd backend && cargo run -p api       # Rust backend
cd frontend && bun run dev           # Astro frontend
```

### Production Deploy

```bash
# 1. Set deploy target in .env
#    DEPLOY_HOST=your-vps.example.com
#    DEPLOY_USER=ubuntu
#    DEPLOY_PATH=/opt/fullstackhex

# 2. Ensure ssh-agent has your key
ssh-add ~/.ssh/id_ed25519

# 3. Deploy
make deploy
```

See [docs/INFRASTRUCTURE.md](docs/INFRASTRUCTURE.md) for full production setup including TLS certificates and PostgreSQL backups.

## Documentation

| Doc | Purpose |
|-----|---------|
| [docs/SETUP.md](docs/SETUP.md) | One-command init and tool install |
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | System design, IPC, port mappings |
| [docs/SERVICES.md](docs/SERVICES.md) | Crate layout, API endpoints, health checks |
| [docs/INFRASTRUCTURE.md](docs/INFRASTRUCTURE.md) | Docker Compose reference, volumes, commands |
| [docs/INITIALIZATION.md](docs/INITIALIZATION.md) | Portable template for new projects |
| [docs/INDEX.md](docs/INDEX.md) | Full documentation index |

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for branch naming, PR process, and code style requirements.

## Security

To report a vulnerability, follow the policy in [.github/SECURITY.md](.github/SECURITY.md). Do not open a public issue.

## Quality Automation

- `CI` runs Rust/Python/frontend checks, smoke tests, and security scans.
- `Dependabot` configuration lives at `.github/dependabot.yml`.
- Local secret scanning is configured via `.pre-commit-config.yaml` and `.github/.secrets.baseline`.

## License

[MIT](LICENSE) © 2026 FullStackHex Contributors
