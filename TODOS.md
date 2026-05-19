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

## Phase 6: Testing Coverage ✅

_All 5 items completed. 282 backend tests pass, 118 frontend tests pass, clippy clean._

### 6.1 Note authorization boundary tests
- `backend/api/tests/integration_notes_routes.rs` — verify user A cannot access/modify user B's notes (all return 404)

### 6.2 OAuth callback error path tests
- `backend/auth/src/routes.rs` — extracted `parse_stored_oauth_state()` and `validate_oauth_state_match()` pure functions; 4 unit tests cover valid/invalid JSON, missing provider, provider mismatch, session mismatch, session match

### 6.3 WebSocket `connectLiveStream()` unit tests
- `frontend/tests/vitest/live.vitest.ts` — 13 tests covering connection, backoff, max retries, clean close, malformed JSON, handler errors, reconnect after failure

### 6.4 Fix auth-gating test to test actual implementation
- `frontend/tests/vitest/auth-gating.vitest.ts` — rewritten to mock `fetch('/api/auth/me')`; 10 tests cover 401, network error, disabled auth, user info, refresh interceptor

### 6.5 WebSocket validation logic tests
- `backend/api/src/live.rs` — `validate_ws_connection()` extracted from `ws_handler` for testability; unit tests cover 401 (auth no cookie), 404 (no Redis), 503 (semaphore exhausted), and Permit (happy path)

---

## Corrections from Verification

| Claim | Verdict | Source |
|-------|---------|--------|
| TypeScript 6.0 is pre-release | **Stable** — released 2026-03-23, 6.0.3 latest patch | microsoft/typescript |
| Vitest 4.x is pre-release | **Stable** — 4.0+ are stable releases | npm registry, vitest.dev |
| `.env` committed to git | **False** — gitignored, not tracked | `git check-ignore .env` |
| CSRF cookie `http_only: false` is a bug | **Not a bug** — intentional for double-submit cookie pattern | code pattern |

---

## Phase 7: Post-Review Fixes (from /plan-eng-review)

_Escalated from the feat/0.14.0 eng review. P1 items block ship; P2/P3 are cleanup todos._

### Fixes (in-progress)
- **P1: HMAC cross-stack payload mismatch** — Rust signs `{"user_id","email","name"}` but Python verifies `{"user_id","email","name","timestamp"}`. Every Rust→Python sidecar forward fails HMAC verification. Add `timestamp` to Rust payload.
- **P1: nginx CSP double-header breaks inline scripts** — nginx sets `script-src 'self'` without nonces. Astro adds nonces via `security.csp` config. Two CSP headers → browser enforces intersection → all inline scripts blocked in production. Remove nginx CSP header or align with Astro's nonce-based approach.
- **P1: Python 2 except syntax** — `py-api/app/main.py:180` uses `except ValueError, TypeError:` (Python 2 syntax). Python 3 requires `except (ValueError, TypeError):`. Crashes on error path.
- **P1: Rate-limit backoff never escalates** — `rate_limit.rs:210-211` always passes `backoff_params(1)`/`backoff_params(2)` (hardcoded 60s). Five-minute and 30-minute escalation tiers are dead code.
- **P2: Password reset token in URL query param** — leaks via nginx access logs, Referer header, browser history.
- **P2: Sessions not invalidated on password reset** — existing JWTs remain valid after password change.
- **P2: Missing metrics paths** for forgot-password, reset-password, providers in `normalize_route()`.
- **P2: OAuth GET+DEL race condition** — `cache_get` + `cache_delete` should be atomic `GETDEL`.
- **P2: ALLOWED_ORIGIN removed from WS handler** — restore Origin validation for cross-site WS hijacking defense.

### Deferred TODOs
- **Clean up dead feature flags** — `chat_enabled` and `storage_readonly` are loaded from env vars, serialized in health responses, but never checked by any handler/middleware/service. Remove flags, env var validation, and health response fields.
- **Persist CSRF token to sessionStorage** — Login/register response handlers should write `csrf_token` to `sessionStorage` so `getCsrfToken()`'s sessionStorage fallback path actually fires.
- **E2E test user cleanup** — Playwright security E2E tests register timestamped users but never delete them. Accumulates over CI runs.
