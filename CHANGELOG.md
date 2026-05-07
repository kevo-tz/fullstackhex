# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [0.10.0.0] - 2026-05-07

### Added
- **Dashboard auth-gated page**: SSR auth guard at `/dashboard` — redirects to `/login` if unauthenticated
- **Client-side token refresh**: fetch interceptor in `Layout.astro` — auto-refreshes on 401, retries original request
- **Auth Grafana dashboard panels**: 6 new panels — Custom Auth Request Rate, Auth Error Rate by Type, Auth p50/p99 Latency (custom), Auth Errors Cumulative, OAuth Callbacks by Provider
- **Auth metrics**: auth request count, latency, and error rates tracked — `auth_requests_total`, `auth_latency_seconds`, `auth_errors_total` now available in Grafana
- **S3 multipart integration tests**: 5 wiremock-based tests for init/failure/upload 2 parts + complete/abort/abort nonexistent
- **Storage coverage tests**: 11 wiremock integration tests for upload/download/streaming/delete/list
- **Cache tests**: 5 unit tests + 5 `#[ignore]` integration tests for `cache::cache`, 2 `#[ignore]` for `cache::pubsub`
- **Auth password tests**: 2 tests for empty password round-trip and invalid hash error
- **Auth gating vitest tests**: `auth-form.vitest.ts` (30 tests), `auth-gating.vitest.ts` (dashboard SSR guard + client-side gating + token refresh interceptor)
- **ETag XML escaping**: defensive `quick_xml::escape::escape()` in multipart `CompleteMultipartUpload` XML body
- **`token_refresh_total` counter**: new metric for monitoring token refresh success rates in Grafana
- **Auth latency histogram buckets**: more precise auth latency tracking with custom buckets (1ms–5s)

### Changed
- **Auth form redirect**: login now redirects to `/dashboard` instead of `/`
- **Dashboard nav**: brand link points to `/dashboard` for authenticated users

## [0.9.0.0] - 2026-05-06

### Added
- **Frontend auth UI**: login and register pages with form validation, responsive nav with hamburger menu, theme system with dark/light mode toggle
- **OAuth provider discovery**: dynamic detection of configured OAuth providers — login page shows Google and GitHub buttons only when env vars are set
- **Diagnostic panel**: auto-retry with backoff on degraded services, connectivity outage recovery, and visual status indicators for each service card
- **E2E auth tests**: Bun-based end-to-end test suite in `e2e/auth.test.ts` covering register, login, refresh, logout, protected routes, and error handling
- **Grafana auth dashboard**: `monitoring/grafana/dashboards/auth.json` with login success/failure rates, registration activity, token issuance and refresh counts, active sessions
- **Deploy script tests**: bats-core test suite (`tests/deploy/deploy_scripts.bats`) covering blue-green, canary, rollback, lock contention, and cleanup
- **E2E shell test framework**: `tests/e2e.sh` for full-stack integration testing with service health polling and isolated test runs
- **Theme vitest tests**: `frontend/tests/vitest/theme.vitest.ts` covering theme toggle, localStorage persistence, and system preference detection
- **CI e2e job**: new GitHub Actions job that starts backend + frontend with real PostgreSQL and Redis, runs the Bun-based e2e suite
- **SQLx offline check**: CI now verifies `cargo sqlx prepare --check` to catch stale offline metadata
- **Socket integration tests**: CI runs `cargo test -- --ignored` for Python sidecar socket integration

### Fixed
- **Cookie auth mode**: added `tracing::warn!` when `AUTH_MODE=cookie` is unsupported by the current configuration — no silent fallback
- **S3 multipart XML parsing**: switched from `serde_xml_rs` to `quick-xml` for reliable parsing of S3 multipart upload responses
- **Refresh token in auth response**: backend now returns `refresh_token` in login and register responses; frontend wired up for auto-refresh
- **CSO supply chain findings**: resolved critical security audit findings including hardcoded credentials, npm token leakage in `.env.example`, and API key exposure in Grafana dashboard configs
- **Shellcheck SC2155**: fixed `local var=$(cmd)` pattern across 4 scripts — `local` declaration and assignment now on separate lines to prevent exit code masking
- **Duplicate email registration**: DB UNIQUE constraint prevents duplicates but error now returns meaningful message instead of generic "Internal server error"

### Changed
- **Environment validation**: `scripts/validate-env.sh` validates `.env` against `.env.example` for missing keys, `CHANGE_ME` placeholders, and shell syntax errors — wired into `make dev`, `make up`, `make watch`
- **Test coverage**: added 22 new tests across auth routes (`register`/`login`/`providers` validation), storage client (`build_object_url` edge cases, multipart XML body), and python-sidecar (`backoff_for_attempt` boundary cases) — coverage improved from 55% to 64%
- **Project documentation**: updated AUTH.md, REDIS.md, STORAGE.md, DEPLOY.md, ARCHITECTURE.md, INDEX.md, EXAMPLES.md, INFRASTRUCTURE.md, MONITORING.md, SERVICES.md, CI.md for v0.9.0.0
- **Makefile DRY**: extracted shared `run-dev` target from near-duplicate `dev`/`watch` targets; replaced `python3` JSON parsing with `grep`/`sed`; added missing `.PHONY` declarations
- **`.env.example` quoting**: `REDIS_SAVE="900 1 300 10 60 10000"` quoted to prevent shell parse errors on `source`

---

## [0.8.0.0] - 2026-05-05

### Added
- **Authentication system** (`crates/auth/`): JWT access tokens with 15-minute expiry, refresh token rotation, OAuth2 login via Google and GitHub, Argon2 password hashing, CSRF token generation, and bearer/cookie dual-mode auth middleware
- **Redis application layer** (`crates/cache/`): connection pooling with `fred`, get/set cache with TTL, pattern invalidation, sliding-window rate limiting via Lua script, pub/sub helpers, and Redis-backed session store
- **S3-compatible object storage** (`crates/storage/`): streaming upload/download (SigV4 signed), presigned URL generation, multipart upload support, list/delete operations — backed by RustFS or any S3-compatible endpoint
- **Deploy safety scripts**: rollback (`make rollback`), blue-green (`make blue-green`), canary (`make canary` with 10% traffic split), canary promote/rollback, and deploy-verify health polling — all with `flock`-based deploy lock
- **Shared error type** (`crates/domain/`): unified `ApiError` enum with consistent HTTP status mapping (401/403/404/429/503) used across auth, cache, storage, and API crates
- **Database migration system**: `sqlx` migrations with `make migrate` / `make migrate-revert` / `make migrate-status` targets, auto-run on API startup
- **Auth health endpoints**: `/health/redis` and `/health/storage` added to the existing health fan-out
- **Python sidecar HMAC verification**: middleware validates `X-Auth-Signature` header on every request using `SIDECAR_SHARED_SECRET` — rejects requests with missing or invalid signatures
- **Nginx configs**: `nginx/upstream.conf.template` for blue-green upstream switching and `nginx/canary.conf` for `split_clients` traffic routing

### Fixed
- **Storage URL safety**: object keys and prefixes are now URL-encoded via `url::Url` — prevents panics from invalid characters and query-parameter injection in list operations
- **Auth error handling**: database errors no longer leak internal details to clients; user enumeration via distinct error messages eliminated
- **Storage download safety**: added 100 MB size limit to prevent OOM from unbounded S3 object downloads
- **Makefile targets**: `make dev` and `make down-dev` now work correctly after signal-handling fixes
- **Auth SQL compatibility**: UUID columns cast to text in queries to avoid type mismatch errors

### Changed
- **Frontend health aggregation**: updated to include Redis and Storage service cards in the dashboard
- **API test suite**: expanded from ~80 to ~140 tests across auth, cache, storage, domain, and Python sidecar crates
- **Project structure**: `crates/core/` renamed to `crates/domain/` to avoid Rust built-in namespace conflict

---

## [0.7.0.0] - 2026-05-03

### Added
- **Prometheus metrics stack**: `/metrics` endpoint on Rust backend exposes `http_requests_total` counter and `http_request_duration_seconds` histogram with bounded route labels — prevents cardinality explosion from dynamic paths
- **Python sidecar metrics**: `/metrics` endpoint on FastAPI sidecar with `python_requests_total` counter and `python_request_duration_seconds` histogram — proxied through Rust backend at `/metrics/python`
- **Database connection pool monitoring**: background task updates `db_pool_connections` gauge every 15 seconds with idle/active counts — abort handle ensures clean shutdown
- **Grafana dashboards**: 5 dashboards for database, infrastructure, overview, Python sidecar, and SLO tracking — auto-provisioned from JSON files
- **Prometheus alerting**: alert rules for service down, high latency, and database issues — commented out by default (opt-in)
- **Docker Compose monitoring stack**: `compose/monitor.yml` with Prometheus, Grafana, Alertmanager, node-exporter, redis-exporter, and postgres-exporter — joins existing Docker network
- **Production Docker Compose**: `compose/prod.yml` with nginx reverse proxy, TLS termination, resource limits, health checks, and all exporters
- **CI pipeline**: GitHub Actions workflow with Rust, Python, frontend, smoke, infra, and security jobs — Docker Buildx with GHA cache for faster builds
- **Graceful shutdown**: SIGTERM/SIGINT handling in Rust backend with connection draining and background task cleanup

### Fixed
- **Prometheus 3.x compatibility**: moved labels from `scrape_config` level into `static_configs` — Prometheus 3.x removed top-level labels
- **Docker network isolation**: added explicit `fullstackhex-network` name so monitoring compose can join it — prevents network creation conflicts
- **Environment variable loading**: added `dotenvy` to automatically load `.env` for `cargo run` — previously required manual export
- **Shell variable expansion**: use literal `DATABASE_URL` values instead of shell vars that don't resolve outside Docker
- **Metrics security**: restrict `/metrics` endpoint to Docker network (172.20.0.0/16) in nginx config — prevents external access to internal metrics
- **Postgres exporter safety**: use individual `DATA_SOURCE_USER`/`DATA_SOURCE_PASS` vars instead of `DATA_SOURCE_NAME` string — avoids shell injection if password contains special chars
- **Deploy script**: `chmod .env` on remote, try HTTPS then fall back to `-k` for health check

### Changed
- **Python sidecar metrics recording**: added `prometheus-client` dependency with structured metric labels
- **CI caching**: Docker Buildx with GitHub Actions cache for Rust builds — faster CI runs

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
