# FullStackHex TODOs

This document is the canonical project plan. It serves as the starting point for all
development on this project. When picking up work, read this first.

## 0. Performance Budget (Template Baseline)

Define before shipping anything real. These numbers are the target.

| Metric | Target | Measurement |
|--------|--------|-------------|
| `/api/health` p50 latency | < 5ms | `bombardier -c 100 -d 30s http://localhost:8001/health` |
| `/api/health` p99 latency | < 20ms | same |
| Rust → Python sidecar roundtrip | < 2ms local | Rust calls Python over Unix socket |
| Postgres query (simple read) | < 10ms p99 | `sqlx --quiet` timings |
| Frontend TTFB (SSR) | < 100ms | `curl -w "%{time_starttransfer}"` |
| Memory per Rust worker | < 50MB RSS | `ps aux` after warmup |
| Zero-allocation hot paths | true | `cargo flamegraph` on p99 path |

If any target is missed, it becomes a P1 item.

## 1. Infrastructure — Development Setup

### Fix naming inconsistency (P1)
- **What:** Rename `rust-backend/` to `backend/`, update `scripts/install.sh` and all docs
- **Why:** `docs/ARCHITECTURE.md` says `backend/`, `install.sh` creates `rust-backend/` — this contradiction causes confusion on day one
- **Files touched:** `scripts/install.sh`, `docs/ARCHITECTURE.md`, `docs/SETUP.md`, `docs/SERVICES.md`, `docs/INFRASTRUCTURE.md`, README, CONTRIBUTING.md, all compose files
- **Acceptance:** `ls` output matches `docs/ARCHITECTURE.md` directory diagram

### Docker Compose organization (P2)
- **What:** Move compose files to `compose/` folder: `compose/dev.yml`, `compose/monitor.yml`, `compose/prod.yml`
- **Why:** Root has 3 YAML files plus root-level Dockerfiles — clean it up before it grows
- **Files touched:** `docker-compose.*.yml` → `compose/*.yml`, CI references, docs
- **Acceptance:** `docker compose -f compose/dev.yml up -d` works identically

### `install.sh` coverage audit (P2)
- **What:** Map every folder created by `install.sh`, verify it matches docs, fix mismatches
- **Why:** Original TODO item #1 was raised but never scoped — make it concrete
- **Acceptance:** Fresh clone + `install.sh` produces: `backend/`, `frontend/`, `python-sidecar/`, `migration/` (for sqlx), `tests/` directories

## 2. Infrastructure — Production Setup

### `production/` folder structure (P1)
- **What:** All production artifacts inside `production/`: Quadlet units, `release.sh`, `rollback.sh`, `.env.prod.example`
- **Why:** Self-contained deployable unit — scp one folder and the VPS is configured
- **Acceptance:** `production/` is the only folder that touches production systems
- **Depends on:** Naming fix (must use `backend/`, not `rust-backend/`)

### Podman secrets (P1)
- **What:** Replace all production `.env` file references with `podman secret create`
- **Why:** `.env` files end up in git history, accidentally shared, or lost. Podman secrets are ephemeral and scoped to the service
- **Files touched:** `production/`, `docker-compose.prod.yml`, docs
- **Acceptance:** `podman secret ls` shows `POSTGRES_PASSWORD`, `RUSTFS_SECRET_KEY`, etc. — zero `.env` files on VPS

### Podman Quadlet deployment (P2)
- **What:** Convert `docker-compose.prod.yml` to Podman Quadlet + systemd units
- **Why:** Rootless Podman on VPS is the production target per original TODOs
- **Files touched:** `production/*.quadlet`, systemd unit files, `compose/prod.yml` (Podman-native variant)
- **Acceptance:** `systemctl --user start fullstackhex-api` starts the service, logs to `journalctl`

### CI → VPS push pipeline (P2)
- **What:** GitHub Actions rsync's `production/` to VPS on tagged release
- **Why:** Currently no automated path from code to running production server
- **Acceptance:** Tag `v0.2.0` → CI builds → CI rsyncs to VPS → VPS applies Quadlet units
- **Depends on:** `production/` folder structure

### Rollback automation (P1)
- **What:** `production/rollback.sh` reverts to previous tagged release artifact
- **Why:** Determines how scary deploying is. If rollback is manual, deploys become scary and get deferred
- **Acceptance:** After failed `release.sh`, `rollback.sh` is triggered automatically, GitHub issue is opened with failure summary
- **Depends on:** `production/` folder, CI → VPS pipeline

## 3. Rust Runtime Tuning (High-Performance P0)

These are what separates a toy from a production system. Address before first load test.

### Tokio worker pool sizing (P1)
- **What:** Configure tokio runtime with explicit worker and spawn-thread counts. Make them tunable via `RUST_TOKIO_WORKERS` and `RUST_TOKIO_THREADS`
- **Why:** Default tokio multi-thread runtime uses all CPU cores — fine locally, wrong in containerized environments where you want 1:1 or 2:1 with container CPU limit
- **Files:** `backend/crates/api/src/main.rs`
- **Acceptance:** `RUST_TOKIO_WORKERS=4 cargo run -p api` uses exactly 4 tokio workers

### Database connection pool (P1)
- **What:** Configure sqlx pool size explicitly. Default (5 connections) is too small for concurrent requests; autotune based on `WORKERS * 2` as a starting point
- **Why:** Connection starvation under load — requests queue behind a saturated pool, latency spikes. Pool exhaustion is a silent killer
- **Files:** `backend/crates/db/src/pool.rs` (create this module), referenced in `main.rs`
- **Acceptance:** `cargo sqlx migrate verify` passes. Pool size is logged at startup. Pool metrics exposed via `/metrics`
- **Context:** Target: `workers * 2` connections, max `workers * 4`, with a semaphore backpressure if pool is exhausted

### Graceful shutdown (P1)
- **What:** Handle `SIGTERM` in both Rust API and Python sidecar. Rust drains in-flight requests (30s timeout), then signals Python to drain and exit
- **Why:** SIGTERM without graceful shutdown = dropped in-flight requests, corrupted state, clients retrying blindly
- **Files:** `backend/crates/api/src/main.rs`, `backend/crates/python-sidecar/src/lib.rs`, `python-sidecar/app/main.py`
- **Acceptance:** `kill -TERM $(pid of api)` → Rust stops accepting new requests, finishes in-flight, SIGTERMs Python, exits cleanly. Log output confirms drain.
- **Diagram:**

```
SIGTERM received
  │
  ├─► Rust: stop accepting new connections
  │     └─► drain in-flight (max 30s)
  │           ├─► success → SIGTERM Python
  │           │     └─► Python drains → exit 0
  │           └─► timeout → SIGKILL Python → exit 1
  │
  └─► Rust: exit 0 (or 1 if drained timeout)
```

### Circuit breaker for Python sidecar (P2)
- **What:** Rust sidecar crate wraps socket calls in a circuit breaker (ratelimit or opentelemetry-semconv pattern). After N failures in a window, open circuit and fail fast for M seconds
- **Why:** A crashing Python sidecar will cascade — Rust waits on dead socket until timeout. Circuit breaker stops the cascade and lets Rust stay healthy
- **Files:** `backend/crates/python-sidecar/src/circuit.rs`
- **Acceptance:** Python crashes → first N requests fail fast (circuit open), M seconds later → half-open → if Python is back, circuit closes. Latency during open-circuit: < 1ms

### Connection pooling — Postgres keepalive (P2)
- **What:** Set `pool.acquire_timeout()`, `pool.idle_timeout()`, and `pool.max_lifetime()`. Configure server-side `tcp_keepalives_idle` awareness
- **Why:** Idle connections drop silently under NAT, load balancers, or Docker networking. App thinks it has connections, Postgres killed them. Queries fail with mysterious "connection closed"
- **Files:** `backend/crates/db/src/pool.rs`
- **Acceptance:** `docker compose restart postgres` → Rust detects dead connections, recreates pool transparently, no 500s visible to clients

## 4. Observability (High-Performance P1)

### Structured logging (P1)
- **What:** Rust backend emits JSON logs (one object per line)
- **Why:** Production log aggregation (Loki, CloudWatch, etc.) needs structured logs. Currently logs are unstructured text.
- **Files:** `backend/crates/api/src/main.rs`, `backend/crates/core/`
- **Acceptance:** `cargo run -p api 2>&1 | jq .` parses every log line without errors

### Prometheus metrics (P1)
- **What:** Expose `/metrics` endpoint with `axum-prometheus` or manual `metrics.rs` crate. Track: request latency histogram, request counter by route/method/status, connection pool utilization, Python sidecar circuit state
- **Why:** You cannot tune what you cannot measure. p50/p95/p99 latency, pool pressure, and circuit state are the first things to look at when latency spikes
- **Files:** `backend/crates/api/src/metrics.rs` (new), wired in `main.rs`
- **Acceptance:** `curl http://localhost:8001/metrics` returns Prometheus format with at minimum: `http_requests_total`, `http_request_duration_seconds`, `db_pool_connections_active`, `sidecar_circuit_state`
- **Histogram buckets:** `[0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0]` seconds — covers sub-ms to seconds

### Load testing baseline (P1)
- **What:** `k6` or `bombardier` script that hits `/api/health` and `/api/python/health` at increasing concurrency. Record baseline before and after every performance change
- **Why:** Without a baseline, performance work is guesswork. Every significant change (pool size, worker count, sidecar circuit) should be regression-tested
- **Files:** `scripts/bench.sh`, `scripts/bench.py-sidecar.sh`
- **Acceptance:** CI runs baseline on tagged releases, compares against previous baseline, fails if p99 regresses > 20%

### Post-deploy smoke test (P2)
- **What:** After `release.sh` completes, curl `https://domain/api/health` 3 times over 30s
- **Why:** A deploy that doesn't verify itself is a deploy that lied to you
- **Files:** `production/release.sh`
- **Acceptance:** Script exits non-zero if smoke test fails, zero if healthy. Output shows RTT per check.

## 5. Testing

### IPC boundary tests (P1)
- **What:** Test Rust ↔ Python Unix socket communication — malformed payloads, large payloads, slow consumers, Rust kills Python
- **Why:** This is the highest-failure path in the architecture with zero test coverage. Rust spawns a subprocess over a socket — what happens when Python crashes?
- **Files:** `backend/crates/python-sidecar/tests/socket_boundary_tests.rs`
- **Acceptance:** `cargo test -p python-sidecar` covers: happy path, Python crash, Python slow response, invalid HTTP response, socket permission denied

### E2E health chain test (P2)
- **What:** Single test that hits `localhost:4321/api/health` and traces the full chain to Python
- **Why:** `docs/ARCHITECTURE.md` describes the flow, `docs/SERVICES.md` maps it out, but nothing verifies it works end-to-end
- **Files:** `frontend/tests/e2e-health.test.ts`
- **Acceptance:** CI runs this as part of the test suite, covers: happy path, Rust unreachable, Python unreachable, invalid JSON response

### SSR proxy error handling (P2)
- **What:** Tests for `frontend/src/pages/api/health.ts` — Rust returns 500, Rust times out, Rust returns non-JSON
- **Why:** Users see what happens when the backend is down — the proxy route needs defined behavior for each failure mode
- **Files:** `frontend/tests/api-proxy-errors.test.ts`
- **Acceptance:** All 3 error scenarios produce a defined HTTP response with appropriate status code

### sqlx migration CI gate (P2)
- **What:** CI verifies `cargo sqlx migrate verify` passes before merge
- **Why:** Broken migrations break production databases. This gate should exist before the first real migration ships
- **Files:** `.github/workflows/ci.yml`
- **Acceptance:** `cargo sqlx migrate verify` runs in CI and fails the build on bad migrations

## 6. Developer Experience

### `justfile` for common commands (P2)
- **What:** `just dev`, `just test`, `just db-reset`, `just logs`, `just db-shell`
- **Why:** `docs/SETUP.md` shows 3-4 manual commands for every operation. `just dev` reduces friction for daily use
- **Files:** `justfile` at repo root
- **Acceptance:** `just dev` starts Docker infra + backend + frontend in one command

### Architecture Decision Records (P3)
- **What:** `docs/adr/0001-python-sidecar-pattern.md` — why Rust manages Python lifecycle, why Unix socket not gRPC
- **Why:** Future contributors will ask "why this?" The ADR is the answer, not a Slack thread from 2026
- **Files:** `docs/adr/` directory with template
- **Context:** Consider ADR for: sidecar pattern, Tailwind v4 (no config file), Astro SSR vs static, circuit breaker choice

## 7. Security & Operations

### Backup runbook (P3)
- **What:** Document backup strategy for Postgres, Redis, RustFS volumes
- **Why:** Persistent data has no backup policy documented. If the VPS disk dies, what is lost?
- **Files:** `docs/ops/backup.md` or `production/README.md`
- **Acceptance:** Script or cron command for each service that can be run without reading source

### Secrets rotation note (P3)
- **What:** Add one paragraph to `production/README.md`: how to rotate secrets with `podman secret`
- **Why:** Operational clarity on what to do when a credential leaks
- **Files:** `production/README.md`
- **Acceptance:** Anyone rotating a secret can do it in 2 commands without reading docs

### Resource limits in production (P2)
- **What:** Podman Quadlet units and docker-compose.prod.yml set memory and CPU limits on all services
- **Why:** Without limits, a memory leak in Python or a query in Postgres can OOM the entire VPS. Limits make the OOM killer predictable
- **Files:** `production/*.quadlet`, `compose/prod.yml`
- **Acceptance:** Each service has explicit `Memory=` and `CPUWeight=` in Quadlet. Postgres: memory=512MB, CPUWeight=100. Redis: memory=256MB, CPUWeight=50. Python: memory=128MB, CPUWeight=200. Rust: memory=256MB, CPUWeight=300

### Connection pool metrics (P2)
- **What:** Expose `db_pool_connections_active`, `db_pool_connections_idle`, `db_pool_connections_timeout_total` on `/metrics`
- **Why:** Pool saturation is the #1 cause of latency spikes. You need to see pool pressure in real-time to tune pool size
- **Files:** `backend/crates/db/src/pool.rs`
- **Acceptance:** `curl :8001/metrics | grep db_pool` returns all three metrics

---

## Priority Summary

| Priority | Items | Key Focus |
|----------|-------|-----------|
| P0 | 1 | Performance budget — define targets before any code ships |
| P1 | 7 | Naming fix, production/, Podman secrets, rollback, graceful shutdown, worker pool sizing, db pool, structured logs, metrics, IPC tests |
| P2 | 8 | Compose org, install audit, CI→VPS, circuit breaker, Postgres keepalive, SSR errors, sqlx gate, post-deploy smoke, justfile, resource limits |
| P3 | 3 | ADRs, backup runbook, secrets rotation |

**Total: 19 items across 7 areas**

## Dependency Graph

```
P0: Performance Budget
  │
  ▼
P1: Tokio worker sizing ────────────────────────► P2: Load testing baseline
P1: DB connection pool                         P2: Circuit breaker
P1: Graceful shutdown                          P2: Connection pool metrics
P1: Structured logging ──────────────────────► P1: Prometheus metrics (depends on logging)
P1: IPC boundary tests                         P2: SSR proxy errors
                                               P2: sqlx migration CI gate
                                               P2: Post-deploy smoke test

Section 1 (infra dev): Naming fix ──────────────────────────► Section 2 (infra prod)
Section 1 (infra dev): Compose org
Section 1 (infra dev): install.sh audit

Section 2 (infra prod): production/ folder ─────────────────► P2: CI→VPS pipeline
Section 2 (infra prod): Podman secrets ─────────────────────► Section 2: Quadlet deployment
Section 2 (infra prod): Podman secrets ─────────────────────► P1: Rollback automation

Section 6 (DX): justfile
Section 7 (sec/ops): Backup runbook ─────────────────────────► Section 7: Secrets rotation
Section 7 (sec/ops): Resource limits ───────────────────────► Section 2: Quadlet deployment
```

## Parallelization Lanes

| Lane | Steps | Shared modules |
|------|-------|----------------|
| A | Section 1 (dev infra) — Naming + Compose org + install audit | — |
| B | Section 3 (Rust runtime) — workers + pool + graceful shutdown + circuit breaker + keepalive | backend/crates/api, backend/crates/db |
| C | Section 4 (observability) — structured logs + metrics + load testing | — |
| D | Section 5 (testing) — IPC boundary + E2E health + SSR errors + sqlx gate | — |
| E | Section 2 (production infra) — production/ + secrets + Quadlet + CI→VPS + rollback | — |
| F | Section 6 (DX) + Section 7 (sec/ops) — justfile, ADRs, backup, secrets rotation, resource limits | — |

**Execution:** A runs first (unblocks E). B, C, D can run in parallel after A. E runs after B (needs backend to exist). F is independent throughout.

(End of file — total 312 lines)