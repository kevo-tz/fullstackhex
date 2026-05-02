# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [0.6.0.0] - 2026-05-02

### Added
- **Structured JSON logging** across all three languages: Rust (`tracing-subscriber` JSON layer), Python (`JsonFormatter` on stderr), and TypeScript (`jsonLog()` wrapper on stdout) — every log line is a JSON object with `timestamp`, `level`, `target`, and `message` fields
- **trace_id propagation** end-to-end: frontend generates UUIDv7 per poll cycle, Rust forwards `x-trace-id` header to Python sidecar, Python extracts and logs it in both health handler and middleware trace — developers can trace a single dashboard refresh across all three services
- **`make dev` DX chain**: before starting services, `check-env` validates `.env`, `check-prereqs` verifies `bun`/`uv`/`cargo`/`docker` are installed, `preflight` detects port/socket conflicts and cleans up stale sockets, `verify-health` polls health endpoints every 1s for up to 30s — all three must be OK before the dashboard URL is printed
- **Cleanup trap**: SIGINT/SIGTERM in `make dev`/`make watch` triggers `make down-dev` via PID_FILE, killing all child processes and Docker containers
- **Socket CI support**: Python sidecar starts as a CI background step; socket integration tests run via `make test-socket-ci`
- **Dashboard vitest tests** (jsdom): 13 tests covering initial loading state, all-green, mixed degradation, all-red, and CSS class transitions — mirrors the inline `setStatus()`/`setDetail()` logic from `index.astro`
- **`docs/logging-conventions.md`**: schema documentation for the structured log format across all three languages

### Changed
- **Rust main.rs** switched from `println!` to structured JSON tracing (`tracing-subscriber` with `.json()` layer)
- **Python sidecar** logging rewritten: `JsonFormatter` replaces uvicorn handlers, `trace_id_middleware` logs every HTTP request with method, path, status, duration, and trace_id
- **Frontend `aggregateHealth()`** now emits structured JSON logs with per-endpoint status and a summary line with `duration_ms`
- **`make watch`** updated with the same DX chain as `make dev`

### Fixed
- Python `JsonFormatter` timestamp rendered `%f` literally instead of microseconds (`logging.Formatter.formatTime()` delegates to `time.strftime()` which doesn't support `%f`)
- Review findings: trace_id continuity in health endpoints, PID tracking for dev processes, logging hardening across layers

---

## [0.5.0.0] - 2026-05-02

### Added
- **Parallel health checks** in the frontend dashboard: three health endpoints are now fetched simultaneously with `Promise.allSettled` instead of sequentially, cutting worst-case load time from ~15s to ~5s
- **`make watch` target** for hot-reload Rust development: starts all services with `cargo watch` so backend changes recompile automatically
- **`make logs-python` target** documenting where to find Python sidecar logs (stdout of the dev/watch terminal)
- **Log locations table** in `docs/SERVICES.md` mapping each service to its log output
- **Gitleaks custom rules** for project-specific secret patterns (RustFS key, PostgreSQL/Redis/Grafana passwords)

### Changed
- **DRY dev/watch startup** with `START_DEPS` macro: PostgreSQL readiness polling with configurable retries and timeout before starting consumers
- **Frontend deps bumped**: Astro 6.1.9 → 6.2.1

### Fixed
- **Non-root USER** added to all three production Dockerfiles (frontend, Python, Rust)
- **Cargo.lock** removed from `.gitignore` and committed (binary app, not a library crate)
- **Spurious Python 3.14 setup** removed from non-Python CI jobs (frontend and Rust lanes)
- **Script consistency sweep**: removed `CHANGE_ME` password defaults, unified socket paths, fixed test-mode hooks

## [0.4.0.0] - 2026-05-01

### Added
- **Cache-Control headers** on all health endpoints: `no-cache, no-store` prevents stale dashboard indicators
- **Actionable fix suggestions** in every health error response: `fix` field tells the developer exactly what command to run
- **Response size limit** on Unix socket reads (1 MiB): prevents memory exhaustion from a runaway sidecar
- **Socket path getter** (`PythonSidecar::socket_path()`): error messages reference the actual configured path, not a hardcoded one

### Changed
- **Default socket path** standardized to `/tmp/fullstackhex-python.sock`
- **DB connection errors** now include the original `sqlx::Error` detail for faster diagnosis
- **Python sidecar retries** capped at 10 to prevent multi-hour backoff from misconfigured env vars

### Fixed
- Response size boundary check off-by-one: exactly-1-MiB responses are no longer rejected
- `no_cache()` header construction uses infallible `from_static` instead of `.unwrap()`

---

## [0.3.1.0] - 2026-05-01

### Changed
- **Health endpoints now use real connections**: `/health/db` runs `SELECT 1` against PostgreSQL instead of checking `DATABASE_URL` is set; `/health/python` makes an HTTP/1.1 request over the Unix socket instead of checking file existence

### Added
- **PythonSidecar client** (`backend/crates/python-sidecar`): Unix-socket HTTP/1.1 transport with retry/backoff, per-request timeout, and a 5-variant error enum
- **`make dev` and `make down-dev`**: one-command full-stack orchestration (compose + Python sidecar + Rust backend + Astro frontend)
- **`.env` wired into Makefile**: all compose targets read `--env-file .env`

### Fixed
- Dashboard no longer shows stale Python error messages after the sidecar recovers
- PostgreSQL data persists correctly across container restarts (Alpine volume path fixed)
- Python sidecar connection retries no longer risk overflow under repeated failures

### Removed
- Dead stub code in `python-sidecar` and `db` crates

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
