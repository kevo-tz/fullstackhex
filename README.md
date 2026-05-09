# FullStackHex

A production-ready full-stack template combining a Rust/Axum backend, Python FastAPI sidecar (`py-api/`), Astro frontend, and six pre-wired infrastructure components — all using the latest available tooling.

## Stack at a Glance

| Layer | Technology |
|-------|-----------|
| Frontend | Astro + Bun + Tailwind CSS |
| Backend | Rust (Axum, edition 2024) |
| Python API (`py-api/`) | FastAPI + uv, connected via Unix domain socket |
| Cache | Redis 8 |
| Database | PostgreSQL 18 |
| Object storage | RustFS (S3-compatible) |

## Key Features

- **Rust + Python API (`py-api/`)** — Rust backend connects to a running Python FastAPI sidecar via Unix domain socket for low-latency IPC without network overhead.
- **Latest tooling** — Rust stable (edition 2024), Bun (latest), uv (latest), Astro v6.
- **Ships complete** — every source file, config, and test is committed. Clone and run — no scaffolding step.
- **Dev infrastructure via Docker Compose** — PostgreSQL 18, Redis 8, and RustFS spin up with a single command; optional Adminer and Redis Commander behind a `tools` profile.
- **Monitoring stack overlay** — `compose/monitor.yml` adds Prometheus + Grafana with 5 auto-provisioned dashboards (API, DB, Python, Infrastructure, SLOs).
- **Metrics out of the box** — Rust backend exposes `/metrics` with request counters and latency histograms; Python sidecar metrics proxied via `/metrics/python`.
- **Full test suites committed** — Rust/Python/Frontend unit, integration, and smoke tests ship in the repo.
- **Security automation** — local `detect-secrets` pre-commit checks plus CI `gitleaks` scanning.
- **Dependency automation** — Dependabot updates for Rust, Python, frontend, and GitHub Actions.
- **MIT licensed** — permissive license, use freely as a project starter.

## Quick Start

Scaffold a new project with a single command:

```bash
curl -fsSL https://raw.githubusercontent.com/kevo-tz/fullstackhex/main/install.sh | bash
```

The installer validates tooling, copies all source files, renames packages and containers to match your project, installs dependencies (`uv sync`, `bun install`, `cargo build`), runs proof-of-concept checks, and initialises a git repo.

Make scripts executable

```bash
cd <your_project_name>
chmod +x scripts/*.sh
```


### Dev Commands

| Command       | Description                                        |
|---------------|----------------------------------------------------|
| `make dev`    | Start full stack (infra + apps)                    |
| `make watch`  | Start full stack with Rust hot reload              |
| `make down`   | Stop all services                                  |
| `make test`   | Run all test suites (rust + python + frontend)     |
| `make logs`   | Follow all stack logs                              |
| `make bench`  | Run performance benchmarks                         |
| `make status` | Show service status (PID, port, health)            |
| `make clean`  | Reset to fresh state (removes volumes)             |

## Documentation

| Doc | Purpose |
|-----|---------|
| [docs/SETUP.md](docs/SETUP.md) | One-command init and tool install |
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | System design, IPC, port mappings |
| [docs/SERVICES.md](docs/SERVICES.md) | Crate layout, API endpoints, health checks |
| [docs/AUTH.md](docs/AUTH.md) | JWT, OAuth, session, and CSRF configuration |
| [docs/STORAGE.md](docs/STORAGE.md) | S3-compatible object storage reference |
| [docs/REDIS.md](docs/REDIS.md) | Redis caching, rate limiting, session store |
| [docs/DEPLOY.md](docs/DEPLOY.md) | Blue-green, canary, rollback, and deploy safety |
| [docs/EXAMPLES.md](docs/EXAMPLES.md) | Copy-paste patterns for extending the stack |
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
