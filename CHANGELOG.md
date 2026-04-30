# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [0.3.0] - 2026-04-30

### Added
- **Axum HTTP router** (`backend/crates/api/src/lib.rs`): exports `router()` with three health endpoints — `/health`, `/health/db`, `/health/python` — all returning structured JSON
- **`/health`**: returns `{status, service, version}` — always 200
- **`/health/db`**: returns `{status: "ok"}` when `DATABASE_URL` is set, `{status: "error", error: …}` otherwise
- **`/health/python`**: returns `{status: "ok"}` when the Unix socket at `PYTHON_SIDECAR_SOCKET` exists, `{status: "unavailable", error: …}` otherwise
- **Real axum integration tests** (`tests/integration_health_route.rs`): 10 in-process HTTP tests replacing previous stubs; cover 200 responses, JSON shape, Content-Type, env-driven status branches, and 404 for unknown routes
- **`serial_test` v3**: workspace dev-dependency; env-mutating integration tests annotated with `#[serial]` to prevent tokio async concurrency from racing on `DATABASE_URL` / `PYTHON_SIDECAR_SOCKET`
- **`backend/.cargo/config.toml`**: sets `test-threads = 1` as a belt-and-suspenders guard when running integration tests directly via `cargo test -p api`
- **Frontend health aggregator** (`frontend/src/pages/api/health.ts`): server-side Astro API route that calls Rust backend `/health`; dashboard (`index.astro`) now fetches via `/api/health` instead of hitting the backend directly

### Fixed
- Env-var race in `health_db_error_when_no_database_url` test: `tokio::test` tasks interleaved despite `test-threads=1`; resolved with `#[serial]` mutex

---

## [0.2.0] - 2026-04-30

### Added
- **OpenCode GitHub Action** (`opencode.yml`): trigger AI-assisted responses to issue and PR comments via `/oc` or `/opencode` mentions
- **Bash test framework** (`scripts/test/helpers.sh`): reusable test scaffolding with `run_test`, `test_setup`/`test_teardown`, mock helpers (`mock_command`, `mock_network_calls`, `mock_read_file`, `mock_write_file`), and assertion utilities (`assert_equals`, `assert_contains`, `assert_exit_code`, `assert_file_exists`, `assert_command_exists`)
- **Test example suite** (`scripts/test_example.sh`): 13 passing tests demonstrating the test framework against `common.sh` utilities
- **Health check script** (`scripts/verify-health.sh`): checks Rust backend, frontend, PostgreSQL, and Redis reachability with configurable timeout and verbose mode
- **`make check-env` target**: validates `.env` exists and has no `CHANGE_ME` placeholders; `make up` now runs this automatically before starting services
- **`AGENTS.md`**: agent instructions with dev start order, test commands, and code quality rules
- **Dry-run support** for `setup-env.sh` and `install-deps.sh`: `--dry-run` flag prevents any filesystem mutations
- **`mock_network_calls`** helper in `common.sh` for stubbing HTTP calls in tests

### Changed
- **CI bootstrap condition**: all three jobs now check all generated dirs (`backend/`, `python-sidecar/`, `frontend/`) before running `install.sh`, preventing silent failures when any directory is missing
- **CI actions upgraded**: `actions/checkout@v6`, `actions/setup-python@v6`, `actions/cache@v5`, `astral-sh/setup-uv@v8`
- **Socket path default** changed to `~/.fullstackhex/sockets/python-sidecar.sock` (user-isolated); production path documented in `.env.example`
- **Benchmark system simplified** to use Apache Bench only; removed Go dependency and redundant `benchmark.sh`
- **`scripts/config.sh`** password defaults changed from `CHANGE_ME` to empty string (credentials now enforced via `make check-env` / `.env`)
- **Secrets baseline** moved from `.secrets.baseline` to `.github/.secrets.baseline`; `gitleaks.toml` and `detect-secrets` config updated to match
- **Architecture docs** updated to reflect `PythonSidecar` API change (no longer spawns the process; connects to a running sidecar via Unix socket)
- **`.gitignore`** updated: added `.mcp.json`, `.backup/`, `.performance/` output dirs; removed `monitoring/` (now tracked)

### Fixed
- `make help` `@echo` lines using spaces instead of tabs (caused `missing separator` errors)
- `Dockerfile.rust` missing `main.rs` stub (caused linker error during dependency caching)
- Docker builder image bumped to Rust 1.95 (current stable for edition 2024)
- `detect-secrets-hook` no longer scans its own baseline file (self-referential false positive)
- `gitleaks` config path corrected to `.github/gitleaks.toml`
- Various `bench.sh` undefined variable and JSON output bugs
- `setup-rust.sh` `--skip-build` flag and edition handling

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
