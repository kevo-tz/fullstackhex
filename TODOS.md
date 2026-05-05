# TODOS

## Now (this branch / next PR)

### A1. Fix migration 001 format [P0] [S]
**What:** `backend/crates/db/migrations/001_create_users.sql` has `CREATE TABLE` and `DROP TABLE` in one file with golang-migrate comments (`-- +migrate Up/Down`). sqlx runs entire file as up-migration — table created then immediately dropped.
**Fix:** Split into `001_create_users.up.sql` / `001_create_users.down.sql`. Remove `-- +migrate` comments. Verify with `make migrate` on fresh database.
**Files:** `backend/crates/db/migrations/001_create_users.sql`

### S1. Wire HMAC-signed auth headers to Python sidecar [P0] [M]
**What:** Rust backend never sends `X-User-Id`, `X-User-Email`, `X-User-Name`, or `X-Auth-Signature` headers to Python sidecar over Unix socket. Sidecar's HMAC middleware bypassed for all routes.
**Fix:** Compute `HMAC-SHA256(SIDECAR_SHARED_SECRET, "{user_id}|{email}|{name}")` and include user headers on every socket request in `python-sidecar/src/lib.rs`.
**Files:** `backend/crates/python-sidecar/src/lib.rs`, `backend/crates/api/src/lib.rs`

### S2. Logout must invalidate session + blacklist token JTI [P0] [S]
**What:** `POST /auth/logout` is a stub — two TODO comments, returns 204 without destroying session or blacklisting token.
**Fix:** Delete session key from Redis, set `blacklist:{jti}` with 15min TTL, delete `refresh:{token}`, clear session cookie.
**Files:** `backend/crates/auth/src/routes.rs`

### A11. Document `make dev` signal handling [P2] [S]
**What:** `make dev` traps INT/TERM to run `make down-dev`, but `wait` at end means Ctrl+C kills everything. Developers expect it to behave like `docker compose up`.
**Fix:** Add 3-line note to README and Makefile help explaining Ctrl+C behavior and alternative per-service startup commands.
**Files:** `README.md`, `Makefile`

## Next (this milestone)

### S3. Atomic token refresh via Lua script [P1] [M]
**What:** `POST /auth/refresh` does get→delete→set as three separate Redis commands. Concurrent refresh requests can corrupt token state.
**Fix:** Replace with single Lua script that checks old token exists, deletes it, sets new token, returns user_id atomically.
**Files:** `backend/crates/auth/src/routes.rs`, `backend/crates/cache/src/session.rs`

### S4. Progressive brute-force backoff [P1] [M]
**What:** Rate limiting uses fixed window. Spec specified progressive backoff: 5 failures → 60s, 10 → 5min, 20 → 30min.
**Fix:** Track failure count in Redis key `backoff:{ip}:{endpoint}`. Increment on each failed login, set TTL based on threshold.
**Files:** `backend/crates/auth/src/routes.rs`, `backend/crates/cache/src/rate_limit.rs`

### S5. Enable CSRF protection in cookie auth mode [P1] [M]
**What:** `csrf.rs` exists and is tested, but cookie auth mode in `auth/src/middleware.rs` is stubbed with TODO. State-changing endpoints have no CSRF protection when `AUTH_MODE=cookie`.
**Fix:** Wire `csrf::generate()` and `csrf::validate()` into cookie auth path. Set CSRF token in separate cookie, validate `X-CSRF-Token` header.
**Files:** `backend/crates/auth/src/middleware.rs`, `backend/crates/auth/src/routes.rs`

### A3. Add `make sync-env` [P1] [S]
**What:** `.env` is gitignored. When `.env.example` changes, developers must manually diff and merge. No automated drift detection.
**Fix:** Add `make sync-env` that compares `.env` against `.env.example` and prints missing keys with example values. Optionally support `make sync-env --apply`.
**Files:** `Makefile`, `scripts/sync-env.sh`

### A5. Make `make dev` background processes survive terminal detachment [P1] [M]
**What:** `make dev` runs `cargo run -p api &` inside shell script. When terminal closes, Rust backend gets SIGTERM and shuts down.
**Fix:** Use `nohup` or write PID files to `/tmp/fullstackhex-dev/`. Add `make status` to show which services are alive.
**Files:** `Makefile`, `scripts/dev-run.sh`

### A7. Add auth status to health dashboard [P1] [S]
**What:** Frontend dashboard shows 5 service cards but no indication whether auth is enabled. When `JWT_SECRET` missing, auth routes return 404 — user sees "all green" but cannot register/login.
**Fix:** Add Auth card to dashboard showing "enabled"/"disabled" based on `/health`. When disabled, show fix instruction.
**Files:** `frontend/src/pages/index.astro`, `frontend/src/lib/health.ts`

### A9. Add contract tests for frontend health aggregation [P1] [M]
**What:** Frontend tests mock `fetch` and assert on response shape. When backend adds new health endpoints, tests break because they expect exact fetch count.
**Fix:** Generate JSON schema from backend `health()` return types and validate frontend mocks against it. Or add `make test-contract` that spins up backend and runs frontend tests against real `/api/health`.
**Files:** `frontend/tests/integration-health-route.test.ts`, `backend/crates/api/src/lib.rs`

### A12. Add sqlx `query!` compile-time checking [P1] [M]
**What:** UUID-to-String type mismatch in auth routes was caught at runtime (HTTP 500). sqlx `query!` with `offline` feature would have caught it at compile time.
**Fix:** Enable `sqlx/offline` in auth crate. Pre-generate `.sqlx/` query metadata with `cargo sqlx prepare`. Update CI to fail if `.sqlx/` is out of date.
**Files:** `backend/crates/auth/Cargo.toml`, `backend/Cargo.toml`, `.github/workflows/*.yml`

## Later

### A6. Add `make status` [P2] [S]
**What:** After `make dev`, no way to verify which services are alive without manually curling health endpoints.
**Fix:** Add `make status` printing table: Service | PID | Port | Health | Uptime. Read from PID files started by `make dev`.
**Files:** `Makefile`, `scripts/status.sh`
**Depends on:** A5

### A8. Add `make test-e2e` [P1] [L]
**What:** Test suites run in isolation. No verification that backend + frontend + database work together. /qa found auth 500 only by manual curl.
**Fix:** Add Playwright or Bun-based e2e test: start services, register user, login, hit `/auth/me`, verify dashboard. Run in CI on every PR.
**Files:** `e2e/`, `.github/workflows/e2e.yml`, `package.json`
**Depends on:** A5

### A10. Add auth login/register UI [P2] [L]
**What:** v0.8 ships auth backend but frontend has no auth UI. Developers must use curl to test registration/login.
**Fix:** Add `/login` Astro page with email/password form and OAuth buttons. Store JWT in localStorage, show user menu. Gate storage actions behind auth.
**Files:** `frontend/src/pages/login.astro`, `frontend/src/components/AuthForm.astro`

### S6. Streaming S3 upload/download [P1] [L]
**What:** `storage/src/routes.rs` buffers entire request body into `Vec<u8>` before uploading. `storage/src/client.rs::download` returns `Vec<u8>`. Large files cause OOM.
**Fix:** Stream `BodyStream` directly to S3 on upload. Return `Stream` or `impl Body` on download.
**Files:** `backend/crates/storage/src/routes.rs`, `backend/crates/storage/src/client.rs`

### S7. Multipart upload for files > 5MB [P2] [L]
**What:** No multipart upload implementation exists. Spec specified multipart for files larger than 5MB.
**Fix:** Implement S3 multipart: initiate upload, stream parts, complete upload. Add `POST /storage/multipart` route.
**Files:** `backend/crates/storage/src/client.rs`, `backend/crates/storage/src/routes.rs`

### S8. New crate test coverage >80% [P1] [L]
**What:** Auth, cache, OAuth, and storage lack integration tests. Current coverage ~50%.
**Fix:** Add `TestClient` integration tests for auth handlers. Use wiremock/mockito for S3, redis-test or mock for cache, httptest for OAuth.
**Files:** `backend/crates/*/tests/`, `backend/crates/*/Cargo.toml`

### S10. End-to-end shell test [P2] [L]
**What:** No automated e2e test covers full user journey.
**Fix:** Add `tests/e2e.sh`: start stack, register user, login, access protected route, upload file, run deploy, verify health, run rollback.
**Files:** `tests/e2e.sh`

### S11. Auth Grafana dashboard [P2] [M]
**What:** Spec specified auth dashboard with login rates, active sessions, OAuth callback success/fail, token refresh rate, brute-force blocked attempts.
**Fix:** Create `monitoring/grafana/dashboards/auth.json` with Prometheus queries for auth metrics.
**Files:** `monitoring/grafana/dashboards/auth.json`

### S12. docs/AUTH.md [P2] [S]
**What:** No auth documentation exists.
**Fix:** Write setup guide: JWT config, OAuth provider setup, session config, brute-force protection, CSRF notes, Python sidecar HMAC trust.
**Files:** `docs/AUTH.md`

### S13. docs/REDIS.md [P2] [S]
**What:** No Redis documentation exists.
**Fix:** Document caching patterns, session usage, pub/sub, rate limiting, connection pool tuning.
**Files:** `docs/REDIS.md`

### S14. docs/STORAGE.md [P2] [S]
**What:** No storage documentation exists.
**Fix:** Document S3/RustFS setup, bucket config, presigned URLs, multipart upload, streaming.
**Files:** `docs/STORAGE.md`

### S15. docs/DEPLOY.md [P2] [S]
**What:** No deploy safety documentation exists.
**Fix:** Document canary, blue-green, rollback commands, deploy lock, nginx config templates, verify script.
**Files:** `docs/DEPLOY.md`

## Icebox

### S9. bats-core tests for deploy scripts [P2] [M]
**What:** Deploy safety scripts are shell scripts with no automated tests.
**Fix:** Add `tests/deploy/` with bats-core tests. Mock docker compose, nginx, scp, `.deploy-state` file.
**Files:** `tests/deploy/`, `scripts/deploy-*.sh`
**Trigger:** CI starts running deploy scripts

### Run ignored socket tests in CI [P2] [M]
**What:** Start test FastAPI instance as CI background step so `#[ignore]` socket integration tests run automatically.
**Why not now:** Socket tests pass on native Linux but fail on WSL2 due to Unix socket quirks. May be flaky in CI.
**Files:** `python-sidecar/src/lib.rs`, `integration_health_route.rs`
**Trigger:** WSL2 CI support or native Linux CI runner

### Add inline Rust doc examples [P2] [S]
**What:** `///` doc comments on `PythonSidecar::get()`, `PythonSidecar::health()`, and `db::health_check` with usage examples.
**Why not now:** Doc examples can rot if not compiled. Low priority for solo dev.
**Trigger:** First external contributor or user request

### Add concrete examples to docs [P2] [S]
**What:** New `docs/EXAMPLES.md` with copy-paste code blocks showing how to extend the template.
**Why not now:** Examples must be maintained as API evolves. Templates change quickly in v0.x.
**Trigger:** First external contributor or stable v1.0 API
