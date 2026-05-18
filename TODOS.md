# TODO — FullStackHex Improvement Plan

_Generated from repo audit. Verified against source and web research._

**Est. total effort: 17–24 hours**

---

## Phase 1: Security Hardening ✅

_All 10 items completed. 267 backend tests pass, 23/23 py-api tests pass, frontend typecheck + lint pass._

- **1.1 CSP hardening** — Astro 6 `security.csp` with `scriptDirective` + `styleDirective`; nginx fallback updated (removed `unsafe-inline` from script-src, added `frame-ancestors`/`base-uri`/`form-action`)
- **1.2 XSS fix** — `formatDate()` returns `"Unknown date"` on parse failure (not raw input)
- **1.3 Storage key validation** — URL-decode + `Path::ParentDir` rejection + null byte/backslash rejection; applied to all handlers
- **1.4 HMAC replay protection** — `X-Timestamp` (±30s window) + `X-Nonce` (Redis dedup, TTL 60s) in py-api middleware
- **1.5 CSRF cookie Secure** — `cookie_secure: bool` in `AuthConfig`, read from `COOKIE_SECURE` env var (default `true`), wired to 3 CSRF cookie calls
- **1.6 `.expect()` removal** — CSRF `getrandom::fill()` → `Result`, `hmac_sha256()` → `Result`, metrics builder uses `unwrap_or_else` with error logging
- **1.7 OAuth CSRF binding** — JSON value with `provider` + `session_id` in Redis; validates session on callback
- **1.8 GitHub email error handling** — `fetch_github_primary_email()` returns `Result` with `warn!` logging; call site propagates error instead of `unwrap_or_default()`
- **1.9 Security CI** — `.github/workflows/security-ci.yml` with `cargo-deny`, `pip-audit`, `eslint` (runs every PR + weekly)
- **1.10 Playwright tests** — `tests/e2e/playwright/security.spec.ts` with CSP header, cookie Secure flag, and XSS assertions

---

## Phase 2: Architecture & Code Quality (4–5h)

### 2.1 Decouple domain crate from cache/db
- `backend/domain/Cargo.toml:15-16` — remove `cache` and `db` deps
- `backend/domain/src/error.rs:34-80` — move `From<cache::CacheError>` and `From<db::DbError>` impls to `backend/api/src/lib.rs`
- Tests in `error.rs:179-223` also move to api crate

### 2.2 Split `AppState` god struct
**File:** `backend/api/src/lib.rs:63-77` (15 fields)
- Create `HealthState`, `WebSocketState` sub-structs
- Use Axum's `State(sub_state)` extraction per route group

### 2.3 Split `build_router` (~135 lines)
**File:** `backend/api/src/lib.rs:181-316`
- Break into 3–4 functions: `health_routes()`, `auth_routes()`, `storage_routes()`, `notes_routes()`

### 2.4 Replace `std::sync::Mutex` for WS tracking
**File:** `backend/api/src/lib.rs:75`
- `Arc<Mutex<HashMap<...>>>` → `Arc<tokio::sync::RwLock<HashMap<...>>>` or `dashmap::DashMap`

### 2.5 Fix Prometheus label cardinality in Python sidecar
**File:** `py-api/app/main.py:193`
- `request.url.path` → use route template via `request.scope["route"].path` if available
- Fallback: split to `request.url.path.split("/")[1]` to group by top-level path

### 2.6 Move `register_metrics()` into lifespan
**File:** `py-api/app/main.py:63`
- Remove module-level call, move inside `lifespan()` after `setup_logging()`

### 2.7 Replace `asyncio.run()` in tests with `pytest-asyncio`
**File:** `py-api/tests/test_hmac_middleware.py`
- Add `pytest-asyncio` to dev deps, convert 5 sync tests to `async def`

### 2.8 Strip health info disclosure
**File:** `backend/api/src/lib.rs:486-522`
- Return boolean `ok` per service + generic status, log detailed fixes server-side only

---

## Phase 3: Frontend Cleanup & Quick Wins (2h)

### 3.1 Remove unused Tailwind CSS
- `frontend/package.json` — remove `tailwindcss`, `@tailwindcss/vite`
- `frontend/astro.config.mjs` — remove `tailwindcss()` plugin
- `frontend/knip.json` — remove from `ignoreDependencies`

### 3.2 Remove dead `flags.ts` or wire it up
**File:** `frontend/src/lib/flags.ts` (52 lines, zero callers)
- Remove file and all references, OR import `fetchFeatureFlags()` on dashboard and use in UI

### 3.3 Use shared `createRetryController()` instead of inline retry
**File:** `frontend/src/pages/index.astro:112-135`
- Replace inline `startRetry`/`cancelRetry`/`resetRetry` with `import { createRetryController } from "../lib/health"`

### 3.4 Remove `@types/node` from global tsconfig
**File:** `frontend/tsconfig.json:4`
- Remove from top-level `types`, scope to Node-specific tsconfig or per-file imports

### 3.5 Configure ESLint for `.astro` files
**File:** `frontend/eslint.config.mjs:11-12`
- Add `eslint-plugin-astro`, or drop ESLint and rely on `@astrojs/check` alone

### 3.6 Extract duplicated CSRF token retrieval
- `frontend/src/pages/notes/[id].astro:107`
- `frontend/src/pages/notes/create.astro:63`
- Create `frontend/src/lib/csrf.ts`: `export function getCsrfToken(): string { return sessionStorage.getItem("csrf_token") || ""; }`

### 3.7 Add `maxlength` + `required` to note body
**File:** `frontend/src/pages/notes/create.astro:19`
- `<textarea>` — add `maxlength="10000" required`

### 3.8 Add custom 404 page
- Create `frontend/src/pages/404.astro` with basic layout and link home

---

## Phase 4: Infrastructure & Script Fixes (3–4h)

### 4.1 Fix Alertmanager dead webhook
**File:** `compose/monitoring/alertmanager.yml:24`
- `http://webhook:5001/alerts` doesn't exist — replace with `log`-based receiver or add a `webhook` service to `monitor.yml`

### 4.2 Fix `down.sh` pkill safety
**File:** `scripts/down.sh:23-27`
- `pkill -u "$(whoami)"` kills ALL matching processes, not just project ones
- Remove pkill fallback; PID-file-based killing at lines 14-20 is sufficient

### 4.3 Fix Dockerfile.python caching
**File:** `compose/Dockerfile.python:10-13`
- First `COPY py-api/pyproject.toml ./` is overwritten by `COPY py-api/ ./py-api/`
- Fix: copy `pyproject.toml` + `uv.lock`, install deps in non-editable mode, THEN copy source

### 4.4 Fix Dockerfile.frontend `npm install` on bun project
**File:** `compose/Dockerfile.frontend:32`
- `npm install --production` ignores `bun.lock` — copy `node_modules` directly from builder instead

### 4.5 Fix `restore.sh` Redis reload
**File:** `scripts/restore.sh:41`
- `redis-cli DEBUG RELOAD` is dangerous — replace with proper AOF replay or `docker compose restart redis`

### 4.6 Fix `dev.sh` Redis config permissions
**File:** `scripts/dev.sh:68-78`
- `.tmp/redis.conf` contains plaintext password with default umask — add `chmod 600`

### 4.7 Fix Prometheus alert metric name
**File:** `compose/monitoring/alerts.yml:17`
- Verify `http_request_duration_seconds_bucket` matches what Axum `metrics` crate actually emits

### 4.8 Pin Docker base image digests
- `compose/Dockerfile.rust`, `Dockerfile.python`, `Dockerfile.frontend` — all use mutable tags

### 4.9 Fix CI e2e test user cleanup
- `.github/workflows/ci.yml` — cleanup `ws-ci@test.local` between runs, or use unique email per run

---

## Phase 5: Missing Features (2–3h)

### 5.1 Add note editing
- `frontend/src/pages/notes/[id].astro` — add Edit button, inline edit mode, PUT to existing endpoint

### 5.2 Add password reset flow
- `backend/auth/src/routes.rs` — add forgot-password and reset-password endpoints
- `frontend/src/components/AuthForm.astro` — add "Forgot password?" link
- `frontend/src/pages/forgot-password.astro`, `reset-password.astro`

### 5.3 Add social meta tags
**File:** `frontend/src/components/Layout.astro` — add `og:title`, `og:description`, `twitter:card`

---

## Phase 6: Testing Coverage (3–4h)

### 6.1 Note authorization boundary tests
- `backend/api/tests/` — verify user A cannot access/modify user B's notes

### 6.2 OAuth callback error path tests
- `backend/auth/tests/` — invalid/expired state, provider mismatch, token exchange failure, concurrent UPSERT race

### 6.3 WebSocket `connectLiveStream()` unit tests
- `frontend/tests/live.test.ts` — connection, backoff, max retries, cleanup, malformed JSON, event emit

### 6.4 Fix auth-gating test to test actual implementation
- `frontend/tests/vitest/auth-gating.vitest.ts` — replace localStorage mock with `fetch('/api/auth/me')` mock

### 6.5 WebSocket stress/load tests
- `backend/api/tests/` — per-user limits, idle timeout, reconnection, semaphore exhaustion

---

## Corrections from Verification

| Claim | Verdict | Source |
|-------|---------|--------|
| TypeScript 6.0 is pre-release | **Stable** — released 2026-03-23, 6.0.3 latest patch | microsoft/typescript |
| Vitest 4.x is pre-release | **Stable** — 4.0+ are stable releases | npm registry, vitest.dev |
| `.env` committed to git | **False** — gitignored, not tracked | `git check-ignore .env` |
| CSRF cookie `http_only: false` is a bug | **Not a bug** — intentional for double-submit cookie pattern | code pattern |
