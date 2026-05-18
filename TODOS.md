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

## Phase 2: Architecture & Code Quality ✅

_All 8 items completed._

- **2.1 Domain decoupled** — `From<cache::CacheError>`/`From<db::DbError>` moved behind `cache-conv`/`db-conv` features in domain
- **2.2 AppState split** — `HealthState` + `WebSocketState` sub-structs with `FromRef` impls
- **2.3 build_router split** — extracted `health_routes()`, `auth_routes()`, `storage_routes()`; notes routes inlined
- **2.4 WS Mutex→RwLock** — `Arc<Mutex<HashMap>>` → `Arc<RwLock<HashMap>>`
- **2.5 Python Prometheus cardinality** — endpoint label normalized via UUID regex
- **2.6 register_metrics() in lifespan** — moved from module level into lifespan
- **2.7 pytest-asyncio** — all py-api tests converted to async def
- **2.8 Health disclosure** — version/fix/error stripped from responses; axum::serve type fix

---

## Phase 3: Frontend Cleanup & Quick Wins ✅

_All 8 items completed._

- **3.1 Tailwind removed** — deps, plugin, knip ignoreDependencies all cleaned
- **3.2 flags.ts removed** — 52-line dead file + test file deleted
- **3.3 Shared createRetryController** — inline retry replaced with import from `health.ts`
- **3.4 @types/node removed** — global tsconfig types cleaned
- **3.5 eslint-plugin-astro** — .astro files now linted
- **3.6 CSRF token extraction** — `src/lib/csrf.ts` with `getCsrfToken()`, used in both notes pages
- **3.7 Textarea validation** — `maxlength="10000" required` on note body
- **3.8 Custom 404 page** — `src/pages/404.astro` with layout + link home

---

## Phase 4: Infrastructure & Script Fixes ✅

_All 9 items completed._

- **4.1** Fix Alertmanager dead webhook — removed dead webhook receiver, alerts log to stdout
- **4.2** Fix `down.sh` pkill safety — removed `pkill -u "$(whoami)"` lines
- **4.3** Fix Dockerfile.python caching — copy `pyproject.toml`+`uv.lock` first, install deps in non-editable mode, THEN copy source
- **4.4** Fix Dockerfile.frontend `npm install` on bun project — copy `node_modules` directly from builder instead
- **4.5** Fix `restore.sh` Redis reload — replaced `redis-cli DEBUG RELOAD` with `docker compose restart redis`
- **4.6** Fix `dev.sh` Redis config permissions — added `chmod 600` after redis.conf write
- **4.7** Fix Prometheus alert metric name — verified `http_request_duration_seconds_bucket` matches Axum metrics emitter
- **4.8** Pin Docker base image digests — all three Dockerfiles pinned to manifest list digests
- **4.9** Fix CI e2e test user cleanup — timestamped email `ws-ci-${ts}@test.local` per run

---

## Phase 5: Missing Features ✅

_All 3 items completed. 267 backend tests pass, 104 frontend tests pass, clippy clean, astro check clean._

- **5.1** Add note editing — Edit button on detail page, edit page at `/notes/edit/[id]` with pre-populated form and PUT submission, redirects to detail page on success
- **5.2** Add password reset flow — `POST /auth/forgot-password` (Redis token, 1h TTL, rate-limited, no email enumeration), `POST /auth/reset-password` (validates token, updates password hash, deletes token), forgot password link on login form, `/forgot-password` and `/reset-password` pages, dev reset URL in non-production
- **5.3** Add social meta tags — `og:title`, `og:description`, `og:image`, `og:type`, `twitter:card` with configurable `description`/`image`/`ogType` props on Layout

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
