# TODOS

All items active on `feat/0.10` branch. Tags: [priority] [effort].

## Now (this branch)

### A10. Auth UI finalize [P0] [M]
**What:** Finish frontend auth: gated `/dashboard` page, client-side token refresh, logout redirect, vitest component tests. Token refresh interceptor goes in Layout.astro (page-wide), not AuthForm.

**Fix:**
- Add `frontend/src/pages/dashboard.astro` (new, auth-gated): shows user email, logout button. Redirect to `/login` if `fullstackhex_token` missing.
- Modify `AuthForm.astro`: store `fullstackhex_refresh_token` alongside existing tokens, redirect to `/dashboard` on success.
- Modify `Layout.astro`: add token refresh interceptor (intercept `fetch` 401, call `/auth/refresh`, retry). Change logout redirect from `/` to `/login`.
- Add `frontend/tests/vitest/auth-form.vitest.ts` (new): form submit, validation, OAuth provider rendering.

**Files:** `frontend/src/pages/dashboard.astro` (new), `frontend/src/components/AuthForm.astro` (modify), `frontend/src/components/Layout.astro` (modify), `frontend/tests/vitest/auth-form.vitest.ts` (new)

**Depends on:** —

### S7. S3 multipart integration tests [P1] [S]
**What:** Multipart upload lifecycle endpoints exist. Add integration tests with mocked HTTP layer.

**Fix:** Add `wiremock` to `backend/crates/storage/Cargo.toml` `[dev-dependencies]`. Write tests for: init multipart, upload 2 parts, complete (round-trip), abort mid-upload.

**Files:** `backend/crates/storage/Cargo.toml` (modify — add wiremock), `backend/crates/storage/src/client.rs` (modify — add tests)

**Depends on:** —

### S8. Coverage >80% per crate [P1] [L]
**What:** Add tests until each crate exceeds 80% line coverage via `cargo tarpaulin --ignore-tests`.

**Fix:** 25 test functions across 6 modules:
- `storage::client.rs`: 10 tests — upload, download (happy + 404), streaming upload, streaming download, delete, list, create_multipart, upload_part, complete_multipart, abort_multipart
- `cache::cache.rs`: 5 tests — get (hit + miss), set, delete, invalidate_pattern, refresh_token_rotate
- `cache::pubsub.rs`: 2 tests — publish + subscribe round-trip
- `auth::csrf.rs`: 2 tests — generate + validate, reject tampered token
- `auth::oauth.rs`: 3 tests — redirect URL construction, code exchange (mock HTTP), provider discovery
- `auth::password.rs`: 3 tests — hash + verify round-trip, wrong password rejection, empty password rejection

Redis helpers: verify `fred` supports mock pubsub. Fall back to `#[cfg(not(test))]` gate if not.

**Files:** Inline `#[cfg(test)] mod tests {}` in each source file. No new test files.

**Depends on:** S7 (storage tests add wiremock infra S8 reuses)

### S11. Auth Grafana dashboard — custom metrics + panels [P1] [M]
**What:** Add auth-specific Prometheus metrics and corresponding dashboard panels. Existing 9 panels use `http_requests_total` — custom metrics don't appear without new panels.

**Fix:**
- Add counters in `crates/api/src/metrics.rs` and instrument handlers in `crates/auth/src/routes.rs`: `auth_requests_total` (method, path), `auth_login_total` (status), `auth_register_total` (status), `auth_errors_total` (error_type), `auth_latency_seconds` (histogram), `oauth_callbacks_total` (provider), `token_refresh_total` (status)
- Add new panels to `monitoring/grafana/dashboards/auth.json` for each custom metric
- Dashboard provisioned via config — no CI changes needed

**Files:** `backend/crates/api/src/metrics.rs` (modify), `backend/crates/auth/src/routes.rs` (modify), `monitoring/grafana/dashboards/auth.json` (modify)

**Depends on:** A10 (auth routes need handlers to instrument)

## Later (deferred from v0.10)

### Auth route handler integration tests [P2] [M]
**What:** `register`/`login`/`logout`/`refresh`/`me` handlers in `crates/auth/src/routes.rs` have zero integration tests. Current tests cover health endpoints only.
**Why deferred:** Scope was already 4 items for v0.10. Auth route tests are a pre-existing gap, not introduced by v0.10.
**Files:** `backend/crates/api/tests/` or `backend/crates/auth/src/routes.rs`

## Icebox

### S3 auto-promote to multipart [P2] [S]
Route-level content-length check: files >5MB auto-init multipart. S3 API is manual by design, and manual endpoints already exist. No user demand yet.

## Review findings captured from /plan-eng-review

```
Step 0:      Scope accepted — 4 commits, rolled back if CI breaks
Architecture: 4 issues (3 resolved, 1 noted)
Code quality: 4 issues (all in scope of A10 commit)
Tests:        4 findings (1 critical gap: auth route handlers, deferred to Later)
Performance:  2 findings (no blocking issues)
Not in scope: auth route handler integration tests
Failure modes: 0 critical gaps (rollback on CI break is sufficient)
Eng Review verdict: CLEARED — proceed with implementation
```
