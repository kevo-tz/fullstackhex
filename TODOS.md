# TODOS

## Later

### A8. Add `make test-e2e` [P1] [L]
**What:** Test suites run in isolation. No verification that backend + frontend + database work together. /qa found auth 500 only by manual curl.
**Fix:** Add Playwright or Bun-based e2e test: start services, register user, login, hit `/auth/me`, verify dashboard. Run in CI on every PR.
**Files:** `e2e/`, `.github/workflows/e2e.yml`, `package.json`

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
