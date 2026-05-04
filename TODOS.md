# TODOS

## Completed

- Wire PythonSidecar IPC + real DB health checks (v0.3.1.0)
- Parallelize frontend health fetches (v0.5.0.0)
- Add `make watch` target (v0.5.0.0)
- Document log locations per service (v0.5.0.0)
- Fix outdated Python sidecar docs in SERVICES.md (v0.5.0.0)
- Add `make logs-python` target (v0.5.0.0)

## Active (from /qa v0.8 review)

### A1. Fix migration 001 format — sqlx runs entire file as up-migration
**What:** `backend/crates/db/migrations/001_create_users.sql` contains both `CREATE TABLE` and `DROP TABLE` in the same file with golang-migrate style comments (`-- +migrate Up/Down`). sqlx ignores these comments and runs the entire file as an up-migration, causing the table to be created then immediately dropped.
**Fix:** Split into `001_create_users.up.sql` / `001_create_users.down.sql` (sqlx standard). Remove the `-- +migrate` comments. Verify by running `make migrate` on a fresh database.
**Files:** `backend/crates/db/migrations/001_create_users.sql`
**Severity:** Critical — breaks auth on fresh installs.
**Found by:** /qa on 2026-05-04.

### A2. Replace `check-env` placeholder-only validation with required-variable schema
**What:** `Makefile:127` `check-env` only checks for `CHANGE_ME` strings. It does NOT verify that required variables like `JWT_SECRET`, `DATABASE_URL`, or `REDIS_URL` actually exist. Auth was silently disabled because `JWT_SECRET` was present in `.env.example` but missing from the local `.env` — `check-env` passed, backend started, but `/auth/*` returned 404.
**Fix:** Add a `scripts/validate-env.py` (or shell) script that reads `.env.example` for required keys and ensures each exists in `.env` with a non-empty value. Run it in `make dev` and `make up` before starting services.
**Files:** `Makefile`, `.env.example`
**Severity:** High — silent feature disable is worse than a crash.
**Found by:** /qa on 2026-05-04.

### A3. Add `.env` sync command — `make sync-env`
**What:** `.env` is gitignored. When `.env.example` changes (e.g., v0.8 adds auth vars), developers must manually diff and merge. There is no automated way to detect drift.
**Fix:** Add `make sync-env` that compares `.env` against `.env.example` and prints missing keys with their example values. Optionally support `make sync-env --apply` to append missing keys.
**Files:** `Makefile`, new `scripts/sync-env.sh`
**Severity:** Medium — friction on every env change.
**Depends on:** A2.

### A4. Quote `REDIS_SAVE` in `.env.example` and validate shell-safe values
**What:** `.env:18` `REDIS_SAVE=900 1 300 10 60 10000` contains unquoted spaces. When `.env` is sourced via `. ../.env`, the shell parses the spaces as separate commands, producing `../.env: line 18: 1: command not found`. This noise obscures real startup errors.
**Fix:** Quote the value in `.env.example`: `REDIS_SAVE="900 1 300 10 60 10000"`. Add a `make check-env` step that sources `.env` with `set -e` to fail fast on syntax errors.
**Files:** `.env.example`, `Makefile`
**Severity:** Medium — startup noise erodes trust in logs.
**Found by:** /qa on 2026-05-04.

### A5. Make `make dev` background processes survive terminal detachment
**What:** The `make dev` target runs `cargo run -p api &` inside a shell script. When the shell receives signals or the terminal session ends, the Rust backend process gets `SIGTERM` and shuts down (`"received shutdown signal, draining connections"`). Developers lose the backend unexpectedly.
**Fix:** Use `nohup` or `systemd-run --user` (or `s6`, `supervisord`) for each service process. Write PID files to a well-known location (`/tmp/fullstackhex-dev/`) and add `make status` to show which services are alive.
**Files:** `Makefile`, `scripts/dev-run.sh`
**Severity:** Medium — process management is the first thing a new dev hits.
**Found by:** /qa on 2026-05-04.

### A6. Add `make status` — show which services are running and on which ports
**What:** After `make dev`, there is no way to verify which services are actually alive without manually `curl`ing health endpoints or `ps`-grepping.
**Fix:** Add `make status` that prints a table: Service | PID | Port | Health | Uptime. Read from PID files started by `make dev`.
**Example output:**
```
Service          PID     Port    Health    Uptime
Rust API         12345   8001    ok        2m
Frontend         12346   4321    ok        2m
Python Sidecar   12347   (sock)  ok        2m
PostgreSQL       12348   5432    ok        2m
Redis            12349   6379    ok        2m
```
**Files:** `Makefile`, `scripts/status.sh`
**Severity:** Low — quality of life.
**Depends on:** A5.

### A7. Add auth status to the health dashboard
**What:** The frontend dashboard at `/` shows 5 service cards but gives zero indication of whether auth is enabled. When `JWT_SECRET` is missing, auth routes return 404 — the user sees "all green" but cannot register or log in.
**Fix:** Add an `Auth` card to the dashboard (or an inline banner) that shows "enabled" / "disabled" based on `/health` or a dedicated `/auth/status` endpoint. When disabled, show a one-line fix: `JWT_SECRET not set — auth disabled. Add to .env and restart.`
**Files:** `frontend/src/pages/index.astro`, `frontend/src/lib/health.ts`
**Severity:** Medium — silent feature disable is confusing.
**Found by:** /qa on 2026-05-04.

### A8. Add `make test-e2e` — full stack smoke test
**What:** The existing test suites (Rust unit, frontend unit, Python unit) run in isolation. None verify that the backend + frontend + database actually work together. `/qa` found the auth 500 bug only by manually curling endpoints.
**Fix:** Add a Playwright or Bun-based e2e test that:
1. Starts `make dev` (or uses existing running services)
2. Opens `http://localhost:4321`
3. Registers a test user via `/auth/register`
4. Logs in via `/auth/login`
5. Hits `/auth/me` with the token
6. Verifies dashboard shows all 5 services as "ok"
Run in CI on every PR.
**Files:** `e2e/`, `.github/workflows/e2e.yml`, `package.json`
**Severity:** High — prevents regressions like the UUID type mismatch from reaching main.
**Depends on:** A5 (reliable process startup).

### A9. Add contract tests for frontend health aggregation
**What:** `frontend/tests/integration-health-route.test.ts` mocks `fetch` and asserts on the response shape. When the backend adds new health endpoints (redis, storage), the tests break because they expect exactly 3 fetches. There is no automated check that frontend expectations match backend reality.
**Fix:** Generate a JSON schema from the backend `health()` return types (or an OpenAPI spec) and validate frontend mocks against it in CI. Alternatively, add a `make test-contract` that spins up the backend and runs the frontend tests against the real `/api/health` endpoint.
**Files:** `frontend/tests/integration-health-route.test.ts`, `backend/crates/api/src/lib.rs`
**Severity:** Medium — test brittleness slows refactors.
**Found by:** /qa on 2026-05-04.

### A10. Add auth login/register UI to the frontend
**What:** v0.8 ships auth backend but the frontend has no auth UI. Developers must use `curl` to test registration and login. This is fine for an API-first project but not for a "full stack" template.
**Fix:** Add a `/login` Astro page with email/password form and OAuth buttons (Google, GitHub). On success, store the JWT in `localStorage` and show a user menu. On the dashboard, gate storage actions behind auth.
**Files:** `frontend/src/pages/login.astro`, `frontend/src/components/AuthForm.astro`
**Severity:** Low — nice to have for a template.
**Depends on:** A2, A7.

### A11. Document the `make dev` signal handling quirk
**What:** `make dev` traps INT/TERM to run `make down-dev`, but the `wait` at the end means Ctrl+C kills everything including the background `cargo run`. Developers who expect `make dev` to run like `docker compose up` are surprised when the backend dies.
**Fix:** Add a 3-line note to the README and Makefile help:
```
make dev runs services in the foreground. Press Ctrl+C to stop all.
If you need services to survive terminal closure, start them individually:
  make up          # docker services only
  cd backend && cargo run -p api  # backend
  cd frontend && bun run dev      # frontend
```
**Files:** `README.md`, `Makefile`
**Severity:** Low — documentation gap.
**Found by:** /qa on 2026-05-04.

### A12. Add sqlx `query!` compile-time checking for auth routes
**What:** The UUID-to-String type mismatch in `backend/crates/auth/src/routes.rs` was caught at runtime (HTTP 500). sqlx's `query!` macro with the `offline` feature would have caught this at compile time.
**Fix:** Enable `sqlx/offline` in `backend/crates/auth/Cargo.toml`. Pre-generate `.sqlx/` query metadata with `cargo sqlx prepare`. Update CI to fail if `.sqlx/` is out of date.
**Files:** `backend/crates/auth/Cargo.toml`, `backend/Cargo.toml`, `.github/workflows/*.yml`
**Severity:** Medium — prevents data-type regressions.
**Found by:** /qa on 2026-05-04.

## Active (from v0.8 plan audit)

### S1. Wire HMAC-signed auth headers to Python sidecar
**What:** The Rust backend never computes or sends `X-User-Id`, `X-User-Email`, `X-User-Name`, or `X-Auth-Signature` headers when forwarding requests to the Python sidecar over the Unix socket. The sidecar's HMAC middleware is therefore bypassed for all routes.
**Fix:** In `python-sidecar/src/lib.rs`, compute `HMAC-SHA256(SIDECAR_SHARED_SECRET, "{user_id}|{email}|{name}")` and include it plus user headers on every socket request. Add a `compute_auth_signature` helper in the Rust backend.
**Files:** `backend/crates/python-sidecar/src/lib.rs`, `backend/crates/api/src/lib.rs`
**Priority:** P0 — security gap.

### S2. Logout must invalidate session + blacklist token JTI
**What:** `POST /auth/logout` is a stub (TODO comment). It does not destroy the Redis session, blacklist the access token JTI, or delete the refresh token. Users can continue using revoked tokens until expiry.
**Fix:** Implement full logout: delete session key from Redis, set `blacklist:{jti}` with 15min TTL, delete `refresh:{token}`, clear session cookie.
**Files:** `backend/crates/auth/src/routes.rs`
**Priority:** P0 — security gap.

### S3. Atomic token refresh via Lua script
**What:** `POST /auth/refresh` does get→delete→set as three separate Redis commands. Concurrent refresh requests can corrupt token state (token family leak).
**Fix:** Replace with a single Lua script that: checks if old refresh token exists, deletes it, sets new token, returns user_id. Script returns error if old token is missing.
**Files:** `backend/crates/auth/src/routes.rs`, `backend/crates/cache/src/session.rs`
**Priority:** P1 — correctness under concurrency.

### S4. Progressive brute-force backoff
**What:** Rate limiting uses a fixed window. The plan specified progressive backoff: 5 failures → 60s block, 10 failures → 5min, 20 failures → 30min.
**Fix:** Track failure count in Redis key `backoff:{ip}:{endpoint}`. On each failed login, increment count and set TTL based on thresholds. Check backoff before rate limit.
**Files:** `backend/crates/auth/src/routes.rs`, `backend/crates/cache/src/rate_limit.rs`
**Priority:** P1 — security hardening.

### S5. Enable CSRF protection in cookie auth mode
**What:** `csrf.rs` exists and is tested, but cookie auth mode in `auth/src/middleware.rs` is stubbed with a TODO. State-changing endpoints have no CSRF protection when `AUTH_MODE=cookie`.
**Fix:** Wire `csrf::generate()` and `csrf::validate()` into the cookie auth path. Set CSRF token in a separate cookie, validate `X-CSRF-Token` header against it.
**Files:** `backend/crates/auth/src/middleware.rs`, `backend/crates/auth/src/routes.rs`
**Priority:** P1 — security feature.

### S6. Streaming S3 upload/download
**What:** `storage/src/routes.rs` buffers the entire request body into `Vec<u8>` before uploading. `storage/src/client.rs::download` returns `Vec<u8>`. Large files cause OOM.
**Fix:** Change upload to stream `BodyStream` directly to S3. Change download to return `Stream` or `impl Body` instead of `Vec<u8>`.
**Files:** `backend/crates/storage/src/routes.rs`, `backend/crates/storage/src/client.rs`
**Priority:** P1 — performance/correctness.

### S7. Multipart upload for files > 5MB
**What:** No multipart upload implementation exists. The plan specified multipart for files larger than 5MB.
**Fix:** Implement S3 multipart: initiate upload, stream parts, complete upload. Add `POST /storage/multipart` route.
**Files:** `backend/crates/storage/src/client.rs`, `backend/crates/storage/src/routes.rs`
**Priority:** P2 — feature gap.

### S8. New crate test coverage >80%
**What:** Auth routes, cache Redis operations, OAuth HTTP flows, and storage I/O lack integration tests. Current coverage is ~50%.
**Fix:** Add `TestClient` integration tests for auth handlers. Add `wiremock` or `mockito` for S3 client. Add `redis-test` or mock `fred::Client` for cache. Add `httptest` for OAuth provider simulation.
**Files:** `backend/crates/*/tests/`, `backend/crates/*/Cargo.toml`
**Priority:** P1 — test coverage debt.

### S9. bats-core tests for deploy scripts
**What:** Deploy safety scripts (rollback, blue-green, canary) are shell scripts with no automated tests.
**Fix:** Add `tests/deploy/` directory with bats-core tests. Mock `docker compose`, `nginx`, `scp`, and `.deploy-state` file. Test happy path and error handling for each script.
**Files:** `tests/deploy/`, `scripts/deploy-*.sh`
**Priority:** P2 — test coverage debt.

### S10. End-to-end shell test
**What:** No automated end-to-end test covers the full user journey.
**Fix:** Add `tests/e2e.sh` that: starts stack, registers user, logs in, accesses protected route, uploads file, runs deploy, verifies health, runs rollback.
**Files:** `tests/e2e.sh`
**Priority:** P2 — regression prevention.

### S11. Auth Grafana dashboard
**What:** The plan specified an auth dashboard (`monitoring/grafana/dashboards/auth.json`) with login rates, active sessions, OAuth callback success/fail, token refresh rate, and brute-force blocked attempts.
**Fix:** Create dashboard JSON with Prometheus queries for `auth_login_total`, `auth_sessions_active`, `auth_oauth_callback_duration_seconds`, `rate_limit_checks_total`.
**Files:** `monitoring/grafana/dashboards/auth.json`
**Priority:** P2 — observability.

### S12. docs/AUTH.md
**What:** No auth documentation exists.
**Fix:** Write setup guide covering JWT config, OAuth provider setup, session config, brute-force protection, CSRF notes, and Python sidecar HMAC trust.
**Files:** `docs/AUTH.md`
**Priority:** P2 — documentation.

### S13. docs/REDIS.md
**What:** No Redis documentation exists.
**Fix:** Document caching patterns, session usage, pub/sub, rate limiting, and connection pool tuning.
**Files:** `docs/REDIS.md`
**Priority:** P2 — documentation.

### S14. docs/STORAGE.md
**What:** No storage documentation exists.
**Fix:** Document S3/RustFS setup, bucket config, presigned URLs, multipart upload, and streaming.
**Files:** `docs/STORAGE.md`
**Priority:** P2 — documentation.

### S15. docs/DEPLOY.md
**What:** No deploy safety documentation exists.
**Fix:** Document canary, blue-green, rollback commands, deploy lock, nginx config templates, and verify script.
**Files:** `docs/DEPLOY.md`
**Priority:** P2 — documentation.

## Deferred

### Run ignored socket tests in CI
**What:** Start a test FastAPI instance as a CI background step so the 5 `#[ignore]` socket integration tests run automatically.
**Why:** Socket tests never run — they require `--ignored` flag. CI has Python setup but doesn't start a sidecar.
**Pros:** Catches socket regressions automatically. Closes the test coverage gap on the polyglot claim.
**Cons:** Adds ~30s to CI runs. Socket tests are timing-sensitive and may be flaky in CI.
**Context:** Tests in `python-sidecar/src/lib.rs` (4 ignored) and `integration_health_route.rs` (1 ignored). All use mock UnixListener. They pass on native Linux but fail on WSL2 due to Unix socket quirks.
**Depends on:** CI Python setup (already exists).

### Add inline Rust doc examples
**What:** `///` comments on `PythonSidecar::get()`, `PythonSidecar::health()`, and `db::health_check`.
**Why:** rust-analyzer hover shows usage examples directly in the editor. Learn by doing without docs.
**Pros:** Zero friction — developer sees example at the point of use. Updates with code changes.
**Cons:** Doc examples can rot if not compiled (use `#[doc = include_str!("...")]` or keep them simple).
**Depends on:** —

### Add concrete examples to docs
**What:** New `docs/EXAMPLES.md` or section in `docs/SERVICES.md` with copy-paste code blocks.
**Why:** No examples showing how to extend the template (add route, add sidecar endpoint, add page).
**Pros:** Reduces time to first custom feature. Shows the full extension pattern end-to-end.
**Cons:** Examples must be maintained as API evolves.
**Depends on:** —
