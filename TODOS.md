# FullStackHex TODOs

This document is the canonical project plan. It serves as the starting point for all
development on this project. When picking up work, read this first.

## Priority Definitions

- **P0** — Blocking: must be done before next release
- **P1** — Critical: should be done this cycle
- **P2** — Important: do when P0/P1 are clear
- **P3** — Nice-to-have: revisit after adoption/usage data
- **P4** — Someday: good idea, no urgency

## Infrastructure

### Performance Budget (P0)

**What:** Define performance targets before shipping anything real

**Why:** Need measurable targets to optimize against, establishes baseline for all performance work

**Context:** These are baseline numbers for the project. Currently scattered in TODOS.md lines 6-19. Should be enforced via CI gates. Move to `docs/performance-budget.md` for visibility.

**Pros:** Clear targets drive optimization decisions, prevents regressions

**Cons:** Needs CI integration to enforce automatically

**Effort:** S (human) → S (CC+gstack)

**Priority:** P0

**Depends on:** None

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

### Fix naming inconsistency (P1)

**What:** Rename `rust-backend/` to `backend/`, update `scripts/install.sh` and all docs

**Why:** `docs/ARCHITECTURE.md` says `backend/`, `install.sh` creates `rust-backend/` — this contradiction causes confusion on day one

**Context:** Directory is named `rust-backend/` but all documentation refers to `backend/`. Need to `git mv rust-backend backend` and update 8+ references in docs and scripts. Start with the rename, then update: `scripts/install.sh`, `docs/ARCHITECTURE.md`, `docs/SETUP.md`, `docs/SERVICES.md`, `docs/INFRASTRUCTURE.md`, README, CONTRIBUTING.md, all compose files.

**Pros:** Eliminates confusion for new contributors, aligns code with documentation

**Cons:** Touches many files (8+), risk of missing a reference

**Effort:** M (human) → S (CC+gstack)

**Priority:** P1

**Depends on:** None

**Acceptance Criteria:** `ls` output matches `docs/ARCHITECTURE.md` directory diagram

### Docker Compose organization (P2)

**What:** Move compose files to `compose/` folder: `compose/dev.yml`, `compose/monitor.yml`, `compose/prod.yml`

**Why:** Root has 3 YAML files plus root-level Dockerfiles — clean it up before it grows

**Context:** Currently docker-compose files are in root. Move them to `compose/` and update all references in docs and CI.

**Pros:** Cleaner root directory, easier to find compose files

**Cons:** Need to update CI and docs references

**Effort:** S (human) → S (CC+gstack)

**Priority:** P2

**Depends on:** None

**Acceptance Criteria:** `docker compose -f compose/dev.yml up -d` works identically

### `install.sh` coverage audit (P2)

**What:** Map every folder created by `install.sh`, verify it matches docs, fix mismatches

**Why:** Original TODO item raised but never scoped — make it concrete

**Context:** Need to audit what `install.sh` actually creates vs what docs say should exist. Run `install.sh` in a test environment and compare with docs. Fix any mismatches found.

**Pros:** Ensures docs match reality, prevents new contributor confusion

**Cons:** Time-consuming manual audit

**Effort:** M (human) → S (CC+gstack)

**Priority:** P2

**Depends on:** Fix naming inconsistency (need correct directory names)
**Blocked by:** None

**Acceptance Criteria:** Fresh clone + `install.sh` produces: `backend/`, `frontend/`, `python-sidecar/`, `backend/crates/db/migrations/` (for sqlx), `tests/` directories.

### `production/` folder structure (P1)

**What:** All production artifacts inside `production/`: Quadlet units, `release.sh`, `rollback.sh`, `.env.prod.example`

**Why:** Self-contained deployable unit — scp one folder and the VPS is configured

**Context:** Currently production-related files are scattered. Need to create `production/` folder and move all production artifacts there. References in docs need updating.

**Pros:** Single folder to deploy, clean separation of dev vs prod

**Cons:** Need to update all references to moved files

**Effort:** M (human) → S (CC+gstack)

**Priority:** P1

**Depends on:** Fix naming inconsistency (must use `backend/`, not `rust-backend/`)
**Blocked by:** None

**Acceptance Criteria:** `production/` is the only folder that touches production systems

### Podman secrets (P1)

**What:** Replace all production `.env` file references with `podman secret create`

**Why:** `.env` files end up in git history, accidentally shared, or lost. Podman secrets are ephemeral and scoped to the service

**Context:** Migrate from `.env` files to Podman secrets for all production services. Update `production/`, `docker-compose.prod.yml`, and docs. Requires updating deploy process.

**Pros:** More secure, no secrets in git history

**Cons:** Learning curve for Podman secrets, need to update deploy process

**Effort:** M (human) → S (CC+gstack)

**Priority:** P1

**Depends on:** `production/` folder structure
**Blocked by:** None

**Acceptance Criteria:** `podman secret ls` shows `POSTGRES_PASSWORD`, `RUSTFS_SECRET_KEY`, etc. — zero `.env` files on VPS

### Podman Quadlet deployment (P2)

**What:** Convert `docker-compose.prod.yml` to Podman Quadlet + systemd units

**Why:** Rootless Podman on VPS is the production target per original TODOs

**Context:** Create Quadlet files for each service and corresponding systemd units. This is the production deployment mechanism. More files to maintain but native systemd integration.

**Pros:** Native systemd integration, rootless Podman, better for production

**Cons:** Quadlet learning curve, more files to maintain

**Effort:** L (human) → M (CC+gstack)

**Priority:** P2

**Depends on:** `production/` folder structure, Podman secrets
**Blocked by:** None

**Acceptance Criteria:** `systemctl --user start fullstackhex-api` starts the service, logs to `journalctl`

### CI → VPS push pipeline (P2)

**What:** GitHub Actions rsync's `production/` to VPS on tagged release

**Why:** Currently no automated path from code to running production server

**Context:** Set up GitHub Actions workflow that rsyncs `production/` to VPS on tag. Requires SSH access setup in GitHub Secrets.

**Pros:** Automated deploys, less manual work, reproducible builds

**Cons:** Requires VPS SSH key management in GitHub, initial setup overhead

**Effort:** L (human) → M (CC+gstack)

**Priority:** P2

**Depends on:** `production/` folder structure
**Blocked by:** None

**Acceptance Criteria:** Tag `v0.2.0` → CI builds → CI rsyncs to VPS → VPS applies Quadlet units

### Rollback automation (P1)

**What:** `production/rollback.sh` reverts to previous tagged release artifact

**Why:** Determines how scary deploying is. If rollback is manual, deploys become scary and get deferred

**Context:** Create rollback script that can revert to any previous tagged release. Should integrate with CI pipeline for automatic rollback on failure.

**Pros:** Makes deploys less scary, encourages more frequent deploys

**Cons:** Script complexity, needs testing

**Effort:** M (human) → S (CC+gstack)

**Priority:** P1

**Depends on:** `production/` folder structure, CI → VPS pipeline
**Blocked by:** None

**Acceptance Criteria:** After failed `release.sh`, `rollback.sh` is triggered automatically, GitHub issue is opened with failure summary

## Rust Runtime

### Tokio worker pool sizing (P1)

**What:** Configure tokio runtime with explicit worker and spawn-thread counts. Make them tunable via `RUST_TOKIO_WORKERS` and `RUST_TOKIO_THREADS`

**Why:** Default tokio multi-thread runtime uses all CPU cores — fine locally, wrong in containerized environments where you want 1:1 or 2:1 with container CPU limit

**Context:** Currently using default tokio runtime. Need to add configuration for worker threads tunable via environment variables. Start in `backend/crates/api/src/main.rs`.

**Pros:** Proper resource usage in containers, tunable for different environments

**Cons:** Adds configuration complexity, more env vars to manage

**Effort:** S (human) → S (CC+gstack)

**Priority:** P1

**Depends on:** None

**Acceptance Criteria:** `RUST_TOKIO_WORKERS=4 cargo run -p api` uses exactly 4 tokio workers

### Database connection pool (P1)

**What:** Configure sqlx pool size explicitly. Default (5 connections) is too small for concurrent requests; autotune based on `WORKERS * 2` as a starting point

**Why:** Connection starvation under load — requests queue behind a saturated pool, latency spikes. Pool exhaustion is a silent killer

**Context:** Create `backend/crates/db/src/pool.rs` module to configure sqlx pool explicitly. Target: `workers * 2` connections, max `workers * 4`, with semaphore backpressure if pool is exhausted. Pool size depends on Tokio worker count.

**Pros:** Prevents connection starvation, tunable, better error handling

**Cons:** Need to create new module, more configuration

**Effort:** M (human) → S (CC+gstack)

**Priority:** P1

**Depends on:** Tokio worker pool sizing (pool size depends on worker count)
**Blocked by:** None

**Acceptance Criteria:** `cargo sqlx migrate verify` passes. Pool size is logged at startup. Pool metrics exposed via `/metrics`

### Graceful shutdown (P1)

**What:** Handle `SIGTERM` in both Rust API and Python sidecar. Rust drains in-flight requests (30s timeout), then signals Python to drain and exit

**Why:** SIGTERM without graceful shutdown = dropped in-flight requests, corrupted state, clients retrying blindly

**Context:** Implement signal handling in Rust and Python. Rust stops accepting new connections, drains in-flight (max 30s), then SIGTERMs Python. Python drains and exits.

**Pros:** No dropped requests on deploy, clean shutdowns, better user experience

**Cons:** Complex cross-process coordination, more code to maintain

**Effort:** L (human) → M (CC+gstack)

**Priority:** P1

**Depends on:** None

**Acceptance Criteria:** `kill -TERM $(pid of api)` → Rust stops accepting new requests, finishes in-flight, SIGTERMs Python, exits cleanly. Log output confirms drain.

**Diagram:**

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

**What:** Rust sidecar crate wraps socket calls in a circuit breaker. After N failures in a window, open circuit and fail fast for M seconds

**Why:** A crashing Python sidecar will cascade — Rust waits on dead socket until timeout. Circuit breaker stops the cascade and lets Rust stay healthy

**Context:** Create `backend/crates/python-sidecar/src/circuit.rs` with circuit breaker logic. Should integrate with metrics for observability.

**Pros:** Prevents cascade failures, faster failure when Python is down, better resilience

**Cons:** Adds complexity to sidecar calls, more code to maintain

**Effort:** M (human) → S (CC+gstack)

**Priority:** P2

**Depends on:** None

**Acceptance Criteria:** Python crashes → first N requests fail fast (circuit open), M seconds later → half-open → if Python is back, circuit closes. Latency during open-circuit: < 1ms

### Connection pooling — Postgres keepalive (P2)

**What:** Set `pool.acquire_timeout()`, `pool.idle_timeout()`, and `pool.max_lifetime()`. Configure server-side `tcp_keep_alives_idle` awareness

**Why:** Idle connections drop silently under NAT, load balancers, or Docker networking. App thinks it has connections, Postgres killed them. Queries fail with mysterious "connection closed"

**Context:** Update `backend/crates/db/src/pool.rs` with keepalive settings. Prevents silent connection drops.

**Pros:** Prevents silent connection drops, more reliable in cloud environments

**Cons:** More pool configuration, tuning required

**Effort:** S (human) → S (CC+gstack)

**Priority:** P2

**Depends on:** Database connection pool
**Blocked by:** None

**Acceptance Criteria:** `docker compose restart postgres` → Rust detects dead connections, recreates pool transparently, no 500s visible to clients

## Observability

### Structured logging (P1)

**What:** Rust backend emits JSON logs (one object per line)

**Why:** Production log aggregation (Loki, CloudWatch, etc.) needs structured logs. Currently logs are unstructured text.

**Context:** Set up structured logging in Rust. Update `backend/crates/api/src/main.rs` and `backend/crates/core/`.

**Pros:** Better log aggregation, easier debugging, standard format

**Cons:** Need to configure logging format, migration from unstructured

**Effort:** M (human) → S (CC+gstack)

**Priority:** P1

**Depends on:** None

**Acceptance Criteria:** `cargo run -p api 2>&1 | jq .` parses every log line without errors

### Prometheus metrics (P1)

**What:** Expose `/metrics` endpoint with `axum-prometheus` or manual `metrics.rs` crate. Track: request latency histogram, request counter by route/method/status, connection pool utilization, Python sidecar circuit state

**Why:** You cannot tune what you cannot measure. p50/p95/p99 latency, pool pressure, and circuit state are the first things to look at when latency spikes

**Context:** Create `backend/crates/api/src/metrics.rs` and wire it in `main.rs`. Depends on structured logging for consistent labels.

**Pros:** Visibility into system performance, data-driven tuning

**Cons:** Adds new module and dependencies, more code to maintain

**Effort:** L (human) → M (CC+gstack)

**Priority:** P1

**Depends on:** Structured logging
**Blocked by:** None

**Acceptance Criteria:** `curl http://localhost:8001/metrics` returns Prometheus format with at minimum: `http_requests_total`, `http_request_duration_seconds`, `db_pool_connections_active`, `sidecar_circuit_state`

**Histogram buckets:** `[0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0]` seconds — covers sub-ms to seconds

### Load testing baseline (P1)

**What:** `k6` or `bombardier` script that hits `/api/health` and `/api/python/health` at increasing concurrency. Record baseline before and after every performance change

**Why:** Without a baseline, performance work is guesswork. Every significant change (pool size, worker count, sidecar circuit) should be regression-tested

**Context:** Create `scripts/bench.sh` and `scripts/bench-py-sidecar.sh` for load testing. Should integrate with CI for regression detection.

**Pros:** Measurable performance regression detection, data-driven optimization

**Cons:** Need to maintain test scripts, CI integration effort

**Effort:** M (human) → S (CC+gstack)

**Priority:** P1

**Depends on:** None

**Acceptance Criteria:** CI runs baseline on tagged releases, compares against previous baseline, fails if p99 regresses > 20%

### Post-deploy smoke test (P2)

**What:** After `release.sh` completes, curl `https://domain/api/health` 3 times over 30s

**Why:** A deploy that doesn't verify itself is a deploy that lied to you

**Context:** Add smoke test to `production/release.sh` script.

**Pros:** Verifies deploy success, catches broken deploys early

**Cons:** Adds delay to deploy process, network dependency

**Effort:** S (human) → S (CC+gstack)

**Priority:** P2

**Depends on:** `production/` folder structure
**Blocked by:** None

**Acceptance Criteria:** Script exits non-zero if smoke test fails, zero if healthy. Output shows RTT per check.

### Connection pool metrics (P2)

**What:** Expose `db_pool_connections_active`, `db_pool_connections_idle`, `db_pool_connections_timeout_total` on `/metrics`

**Why:** Pool saturation is the #1 cause of latency spikes. You need to see pool pressure in real-time to tune pool size

**Context:** Add metrics to existing `backend/crates/db/src/pool.rs`. Complements Prometheus metrics.

**Pros:** Visibility into pool health, easier tuning

**Cons:** Minor addition to pool module, more metrics to monitor

**Effort:** S (human) → S (CC+gstack)

**Priority:** P2

**Depends on:** Database connection pool, Prometheus metrics
**Blocked by:** None

**Acceptance Criteria:** `curl :8001/metrics | grep db_pool` returns all three metrics

## Testing

### IPC boundary tests (P1)

**What:** Test Rust ↔ Python Unix socket communication — malformed payloads, large payloads, slow consumers, Rust kills Python

**Why:** This is the highest-failure path in the architecture with zero test coverage. Rust spawns a subprocess over a socket — what happens when Python crashes?

**Context:** Create `backend/crates/python-sidecar/tests/socket_boundary_tests.rs` with comprehensive IPC tests. Complex test setup (spawning Python process).

**Pros:** Catches IPC failures before production, better reliability

**Cons:** Complex test setup, requires full stack running

**Effort:** L (human) → M (CC+gstack)

**Priority:** P1

**Depends on:** None

**Acceptance Criteria:** `cargo test -p python-sidecar` covers: happy path, Python crash, Python slow response, invalid HTTP response, socket permission denied

### E2E health chain test (P2)

**What:** Single test that hits `localhost:4321/api/health` and traces the full chain to Python

**Why:** `docs/ARCHITECTURE.md` describes the flow, `docs/SERVICES.md` maps it out, but nothing verifies it works end-to-end

**Context:** Create `frontend/tests/e2e-health.test.ts` for E2E testing. Requires full stack running.

**Pros:** Verifies full request chain, catches integration issues

**Cons:** Requires full stack running for tests, slower test execution

**Effort:** M (human) → S (CC+gstack)

**Priority:** P2

**Depends on:** None

**Acceptance Criteria:** CI runs this as part of the test suite, covers: happy path, Rust unreachable, Python unreachable, invalid JSON response

### SSR proxy error handling (P2)

**What:** Tests for `frontend/src/pages/api/health.ts` — Rust returns 500, Rust times out, Rust returns non-JSON

**Why:** Users see what happens when the backend is down — the proxy route needs defined behavior for each failure mode

**Context:** Create `frontend/tests/api-proxy-errors.test.ts` for error handling tests. Requires mocking backend failures.

**Pros:** Defined behavior on backend failures, better user experience

**Cons:** Requires mocking backend failures, more test code

**Effort:** M (human) → S (CC+gstack)

**Priority:** P2

**Depends on:** None

**Acceptance Criteria:** All 3 error scenarios produce a defined HTTP response with appropriate status code

### sqlx migration CI gate (P2)

**What:** CI verifies `cargo sqlx migrate verify` passes before merge

**Why:** Broken migrations break production databases. This gate should exist before the first real migration ships

**Context:** Add migration verification to `.github/workflows/ci.yml`.

**Pros:** Prevents broken migrations from merging, protects production data

**Cons:** Adds CI step, may slow down CI slightly

**Effort:** S (human) → S (CC+gstack)

**Priority:** P2

**Depends on:** None

**Acceptance Criteria:** `cargo sqlx migrate verify` runs in CI and fails the build on bad migrations

## Developer Experience

### `justfile` for common commands (P2)

**What:** `just dev`, `just test`, `just db-reset`, `just logs`, `just db-shell`

**Why:** `docs/SETUP.md` shows 3-4 manual commands for every operation. `just dev` reduces friction for daily use

**Context:** Create `justfile` at repo root with common commands. Simple tool, easy to learn.

**Pros:** Reduces daily friction, standardizes commands across team

**Cons:** Another tool to learn (but just is simple), maintenance overhead

**Effort:** S (human) → S (CC+gstack)

**Priority:** P2

**Depends on:** None

**Acceptance Criteria:** `just dev` starts Docker infra + backend + frontend in one command

### Architecture Decision Records (P3)

**What:** `docs/adr/0001-python-sidecar-pattern.md` — why Rust manages Python lifecycle, why Unix socket not gRPC

**Why:** Future contributors will ask "why this?" The ADR is the answer, not a Slack thread from 2026

**Context:** Create `docs/adr/` directory with ADR template. Consider ADRs for: sidecar pattern, Tailwind v4 (no config file), Astro SSR vs static, circuit breaker choice.

**Pros:** Documents architectural decisions, helps future contributors

**Cons:** Time to write ADRs, maintenance overhead

**Effort:** M (human) → S (CC+gstack)

**Priority:** P3

**Depends on:** None

**Acceptance Criteria:** ADR template created, first ADR written for Python sidecar pattern

## Security & Operations

### Backup runbook (P3)

**What:** Document backup strategy for Postgres, Redis, RustFS volumes

**Why:** Persistent data has no backup policy documented. If the VPS disk dies, what is lost?

**Context:** Create `docs/ops/backup.md` or add to `production/README.md`.

**Pros:** Disaster recovery plan, protects against data loss

**Cons:** Documentation effort, needs maintenance as architecture changes

**Effort:** S (human) → S (CC+gstack)

**Priority:** P3

**Depends on:** None

**Acceptance Criteria:** Script or cron command for each service that can be run without reading source

### Secrets rotation note (P3)

**What:** Add one paragraph to `production/README.md`: how to rotate secrets with `podman secret`

**Why:** Operational clarity on what to do when a credential leaks

**Context:** Document secret rotation process in production README.

**Pros:** Operational clarity, faster incident response

**Cons:** Documentation effort

**Effort:** S (human) → S (CC+gstack)

**Priority:** P3

**Depends on:** Podman secrets
**Blocked by:** None

**Acceptance Criteria:** Anyone rotating a secret can do it in 2 commands without reading docs

### Resource limits in production (P2)

**What:** Podman Quadlet units and docker-compose.prod.yml set memory and CPU limits on all services

**Why:** Without limits, a memory leak in Python or a query in Postgres can OOM the entire VPS. Limits make the OOM killer predictable

**Context:** Update `production/*.quadlet` and `compose/prod.yml` with resource limits. Need to tune limits per service.

**Pros:** Predictable OOM behavior, protects VPS from resource exhaustion

**Cons:** Need to tune limits per service, may need adjustment over time

**Effort:** M (human) → S (CC+gstack)

**Priority:** P2

**Depends on:** Podman Quadlet deployment
**Blocked by:** None

**Acceptance Criteria:** Each service has explicit `Memory=` and `CPUWeight=` in Quadlet. Postgres: memory=512MB, CPUWeight=100. Redis: memory=256MB, CPUWeight=50. Python: memory=128MB, CPUWeight=200. Rust: memory=256MB, CPUWeight=300

---

## Priority Summary

| Priority | Items | Key Focus |
|----------|-------|-----------|
| P0 | 1 | ✓ Complete — Performance budget targets defined in `docs/performance-budget.md` |
| P1 | 11 | Naming fix ✓, production/, Podman secrets, rollback, graceful shutdown, worker pool sizing, db pool, structured logs, metrics, load testing, IPC tests |
| P2 | 13 | Compose org ✓, install audit, CI→VPS, circuit breaker, Postgres keepalive, SSR errors, sqlx gate, post-deploy smoke, justfile, resource limits, connection pool metrics, E2E health test |
| P3 | 3 | ADRs, backup runbook, secrets rotation |

**Total: 25 items across 6 areas (3 completed)**

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
| A | Infrastructure dev — Naming + Compose org + install audit | — |
| B | Rust runtime — workers + pool + graceful shutdown + circuit breaker + keepalive | backend/crates/api, backend/crates/db |
| C | Observability — structured logs + metrics + load testing + pool metrics | — |
| D | Testing — IPC boundary + E2E health + SSR errors + sqlx gate | — |
| E | Infrastructure prod — production/ + secrets + Quadlet + CI→VPS + rollback | — |
| F | DX + Sec/Ops — justfile, ADRs, backup, secrets rotation, resource limits | — |

**Execution:** A runs first (unblocks E). B, C, D can run in parallel after A. E runs after B (needs backend to exist). F is independent throughout.

## Completed

| Item | Priority | Status | Notes |
|------|---------|--------|-------|
| Performance Budget | P0 | ✓ | Moved to `docs/performance-budget.md`; targets defined (p50 < 5ms, p99 < 20ms, etc.) |
| Fix naming inconsistency | P1 | ✓ | All `rust-backend/` → `backend/` references updated in 18 files |
| Docker Compose organization | P2 | ✓ | Compose files moved to `compose/`; all docs updated |
