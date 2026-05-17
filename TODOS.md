# TODO

## Priority order: P0 (ship-blocking) → P1 (should fix) → P2 (nice to have)

### P0 — Bugs that break features or make production unshippable

- [ ] **T1 (P0, 1 line fix)** — WebSocket `connect()` never called — `frontend/src/lib/live.ts:75`
  - `connect()` defined but never invoked inside `connectLiveStream()`. Live dashboard is dead code — always falls back to HTTP polling.
  - Fix: add `connect();` before the return statement at line 139.
  - Verify: `bun test` or manual check that WS connection initiates on dashboard.

- [ ] **T2 (P0, multi-file)** — TLS/certbot broken in production — `compose/prod.yml:348-355`, `compose/nginx/nginx.conf:45-47,61-62`
  - ACME challenge dir not shared between nginx and certbot. Certificate path mismatch (certbot writes to `live/<domain>/`, nginx expects flat files). No initial certificate acquisition (only `certbot renew`, no `certbot certonly`).
  - Fix: add shared ACME volume, add initial `certbot certonly` entrypoint or init container, add symlink/post-renew hook for cert paths.
  - Verify: `docker compose -f compose/prod.yml up` and confirm HTTPS works.

- [ ] **T3 (P0, 4 files)** — PostgreSQL volume path mismatch dev vs prod — `compose/dev.yml:32` vs `compose/prod.yml:188`
  - Dev mounts at `/var/lib/postgresql` (parent dir), prod mounts at `/var/lib/postgresql/data` (data subdir). Volumes are structurally incompatible — migrating a dev DB to prod silently fails.
  - Fix: standardize to `/var/lib/postgresql/data` in both files (the standard PostgreSQL data directory).
  - Verify: `docker compose down -v && docker compose up` with both configs and confirm DB initializes.

### P1 — Security, correctness, and observability issues

- [ ] **T4 (P1, 2 files)** — JWT in WebSocket query string leaks to logs/history — `backend/api/src/live.rs:107-118,225-239`
  - If Redis is available, exposes JWT via `?token=<jwt>` in upgrade URL → leaked in access logs, Referer headers, browser history.
  - Fix: use short-lived one-time ticket exchanged for the JWT server-side, or require cookie auth for WS exclusively.
  - Verify: WS connects without token in URL; access logs show no JWT.

- [ ] **T5 (P1, 1 file)** — Missing `Secure` flag on all auth cookies — `backend/auth/src/cookies.rs:11-15`
  - Access token, refresh token, session, CSRF cookies all sent over plain HTTP.
  - Fix: add `; Secure` to cookie format strings when `X-Forwarded-Proto: https` or config flag is set.
  - Verify: cookies include `Secure` attribute in production responses.

- [ ] **T6 (P1, 1 file)** — OAuth CSRF token not deleted on provider mismatch — `backend/auth/src/routes.rs:676-683`
  - Early return on provider mismatch skips `cache_delete`. Attacker can replay OAuth state within 10-min TTL.
  - Fix: move `cache_delete` before the provider comparison, or add it to a finally/defer.
  - Verify: OAuth provider mismatch endpoint deletes the CSRF token before returning.

- [ ] **T7 (P1, 2 files)** — Security headers dropped by auth proxy — `frontend/src/pages/api/auth/[...route].ts:37-46`
  - Only `content-type` and `set-cookie` forwarded. HSTS, CSP, X-Content-Type-Options silently dropped.
  - Fix: forward all backend response headers, or explicitly list required security headers.
  - Verify: auth proxy response includes HSTS, CSP, X-Content-Type-Options.

- [ ] **T8 (P1, 1 file)** — Bearer token scheme case-sensitive — `backend/auth/src/middleware.rs:180`
  - RFC 7235 requires case-insensitive auth schemes. `bearer`/`BEARER` rejected.
  - Fix: use `to_lowercase()` comparison or `strip_prefix` on lowercased value.
  - Verify: requests with `authorization: bearer <token>` authenticate successfully.

- [ ] **T9 (P1, 1 file)** — AllowedOrigin hostname case-sensitive — `backend/api/src/live.rs:158`
  - RFC 3986 hostnames are case-insensitive. `Example.com` != `example.com`.
  - Fix: lowercase both sides before comparison.
  - Verify: `Origin: https://Example.com` matches `ALLOWED_ORIGIN: https://example.com`.

- [ ] **T10 (P1, 1 file)** — Python HMAC auth failures invisible to monitoring — `py-api/app/main.py:108,134`
  - Middleware ordering: HMAC rejection returns before trace_id/logging middleware runs. Brute-force produces zero observability.
  - Fix: reverse middleware registration order (trace_id outer, HMAC inner), or add explicit logging/metrics to HMAC rejection branches.
  - Verify: HMAC auth failures produce a log line and increment a Prometheus counter.

- [ ] **T11 (P1, 1 file)** — Dockerfile.python dependency duplication — `compose/Dockerfile.python:14-15`
  - Three deps hardcoded in RUN command AND in `pyproject.toml`. Two sources of truth will diverge.
  - Fix: remove explicit `uv pip install` of deps, let `-e ./py-api/` install pull them from pyproject.toml.
  - Verify: Docker image builds and runs, `uvicorn`/`fastapi` installed at correct versions.

- [ ] **T12 (P1, 1 file)** — WsUserGuard::drop() silently swallows tokio::spawn failures — `backend/api/src/live.rs:308-322`
  - Per-user WS connection counter never decremented on spawn failure. User quota exhaustion requires server restart.
  - Fix: handle spawn result or use a synchronous decrement path.
  - Verify: per-user WS counter decrements correctly after disconnection.

### P2 — Important improvements, less critical

- [ ] **T13 (P2, 1 file)** — `isFullOutage` treats missing entries as healthy — `frontend/src/lib/health.ts:38`
  - Missing service entries should NOT short-circuit to "not an outage".
  - Fix: change `!entry || entry.status === "ok"` to `!entry && entry.status !== "ok"` or similar.
  - Verify: empty health response triggers outage UI.

- [ ] **T14 (P2, 1 file)** — Redis password exposed in `ps` output — `compose/prod.yml:222`
  - `--requirepass` in command line is visible via `docker ps`, `/proc`.
  - Fix: use Redis config file (like dev.yml does: `.tmp/redis.conf`).
  - Verify: Redis password not visible in process list.

- [ ] **T15 (P2, 2 files)** — No CSRF protection on login/register forms — `frontend/src/components/AuthForm.astro:118-124`
  - Standard JSON POST without CSRF token. Vulnerable to CSRF on forms.
  - Fix: include CSRF token from sessionStorage in request.
  - Verify: auth POST requests include CSRF token header.

- [ ] **T16 (P2, 1 file)** — `normalize_route` misses multipart storage routes — `backend/api/src/metrics.rs:72`
  - Multipart complete/part/abort routes all lumped under `"unknown"`.
  - Fix: add route patterns for `/storage/multipart/{key}/{upload_id}/*` variants.
  - Verify: multipart operations appear in metrics with correct route label.

- [ ] **T17 (P2, 2 files)** — `client_ip` trusts unvalidated `X-Forwarded-For` — `backend/auth/src/routes.rs:29-42`
  - If no reverse proxy strips incoming `X-Forwarded-For`, attacker spoofs IP. Default `"unknown"` collapses all clients into one rate limit bucket.
  - Fix: only trust `X-Forwarded-For` when proxy trust is configured; fall back to connection IP. Or always require a trusted proxy to set it.
  - Verify: rate limit keys use real client IP, not `"unknown"`.

- [ ] **T18 (P2, 1 file)** — CSRF token not refreshed after token refresh — `frontend/src/components/Layout.astro:137-177`
  - After token refresh, CSRF token in sessionStorage is stale.
  - Fix: update sessionStorage CSRF token from refresh response.
  - Verify: after token refresh, new CSRF token is used for subsequent requests.

- [ ] **T19 (P2, 1 file)** — `backoff_increment` INCR/EXPIRE race — `backend/cache/src/rate_limit.rs:193-210`
  - Separate INCR and EXPIRE calls. EXPIRE failure → key persists without TTL.
  - Fix: use Lua script or Redis `SET` with EX/PX to atomically set count + TTL.
  - Verify: backoff keys always have TTL regardless of concurrent requests.

- [ ] **T20 (P2, 1 file)** — Alertmanager webhook targets container localhost — `compose/monitoring/alertmanager.yml:24`
  - `http://localhost:5001/alerts` inside Alertmanager container resolves to its own localhost.
  - Fix: use Docker service name or `host.docker.internal`.
  - Verify: alerts reach the configured webhook receiver.

- [ ] **T21 (P2, 1 file)** — `HighLatency` alert over-aggregates all routes — `compose/monitoring/alerts.yml:14-24`
  - `sum(rate(...)) by (le)` collapses all endpoints into one p99. Masked hotspots.
  - Fix: add `job` or `route` to the `by()` clause.
  - Verify: alert fires for individual slow endpoints, not just global latency.

- [ ] **T22 (P2, 2 files)** — Python test monkeypatch is dead code — `py-api/tests/test_hmac_middleware.py:40,57,77,120`
  - `monkeypatch.setenv` runs after `Settings()` is already constructed. Only the direct `shared_secret =` mutation has effect.
  - Fix: remove monkeypatch lines or restructure tests to set env before import.
  - Verify: tests pass and actually use env var for setup.

- [ ] **T23 (P2, 2 files)** — Missing `x-trace-id` logging in HMAC rejection paths — `py-api/app/main.py:159-179`
  - Failed auth paths return `Response` directly without logging the incoming trace_id.
  - Fix: add `logger.warning(...)` with trace_id before each rejection return.
  - Verify: rejected HMAC requests appear in logs with trace_id.

- [ ] **T24 (P2, 1 file)** — `bench.sh` missing `set -u` and `set -o pipefail` — `scripts/bench.sh:2`
  - Unset variables silently become empty strings. Pipeline failures masked.
  - Fix: change to `set -euo pipefail`.
  - Verify: undefined variable references cause immediate failure.

- [ ] **T25 (P2, 1 file)** — `dev.sh` hardcoded compose path for pg_isready — `scripts/dev.sh:85`
  - Uses `docker compose -f compose/dev.yml` instead of `$COMPOSE_DEV` like the rest of the file.
  - Fix: replace with `$COMPOSE_DEV`.
  - Verify: pg_isready check uses the same compose config as the `up` command.

### P3 — Code quality, DX, and minor issues

- [ ] **T26 (P3, 1 file)** — `handleRustHealth` duplicates `handleService` logic — `frontend/src/lib/health.ts:131-165`
  - ~35 lines nearly identical to `handleService` at line 95.
  - Fix: unify into a single parametrized function.
  - Verify: existing tests pass, health dashboard renders same output.

- [ ] **T27 (P3, 1 file)** — `escapeHtml` creates real DOM nodes per call — `frontend/src/pages/notes/index.astro:130-134`
  - Creates `<div>` element for every string. For 20+ notes, 20+ unnecessary DOM allocations.
  - Fix: use string replacement (`&` → `&amp;`, `<` → `&lt;`, etc.) or a reusable element.
  - Verify: all HTML entities properly escaped in rendered notes.

- [ ] **T28 (P3, 1 file)** — Mixed `var`/`let` usage in Layout.astro — `frontend/src/components/Layout.astro:185-197`
  - Uses `var` at lines 186-187, 196-197 while rest of codebase uses `let`/`const`.
  - Fix: replace `var` with `let`/`const`.
  - Verify: no functional change, lint passes.

- [ ] **T29 (P3, 1 file)** — `as any` cast in notes detail page — `frontend/src/pages/notes/[id].astro:88`
  - `document.querySelector("modal-element") as any` bypasses TypeScript safety.
  - Fix: properly type with ModalElement class.
  - Verify: TypeScript checks pass.

- [ ] **T30 (P3, 1 file)** — `backup.sh` SAVE fires before copy success check — `scripts/backup.sh:23-26`
  - Redis SAVE fires (overwriting dump.rdb) before the `cp` command. If `cp` fails, new dump lost AND old dump gone.
  - Fix: capture the copy exit code, check it, log error without `2>/dev/null`.
  - Verify: backup failure produces clear error message, doesn't silently skip.

- [ ] **T31 (P3, 1 file)** — Duplicate `HealthResponse` interface — `frontend/src/lib/flags.ts:9-17` vs `frontend/src/lib/health.ts:20-27`
  - Same name, different shape. Co-import causes TypeScript error.
  - Fix: rename one (e.g., `HealthResponseWithFlags`), or define once and extend.
  - Verify: both files can be imported together without TS errors.

- [ ] **T32 (P3, 1 file)** — Dockerfile.frontend only copies `bun.lock` not `bun.lockb` — `compose/Dockerfile.frontend:7`
  - Bun <1.1 uses `bun.lockb`, >=1.1 uses `bun.lock`. COPY fails if the wrong one exists.
  - Fix: use a conditional COPY or copy both with `|| true`.
  - Verify: Docker build succeeds regardless of Bun lockfile format.
