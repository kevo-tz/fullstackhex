# TODOS

Deep-dive audit findings across Rust backend, Astro frontend, Python API, and infra/docs.
Organized by category with priority tags. Verified against source code on 2026-05-10.
Re-verified on 2026-05-10 — items marked `[x]` are FIXED, `[ ]` are still OPEN.

---

## Bugs

### CRITICAL

- [x] **CSRF bypass when cookie+header both absent** — `validate_csrf_token("", "")` now returns false (csrf.rs:20 rejects empty tokens). FIXED.
- [ ] **`prometheus-client` missing from Dockerfile** — Dockerfile.python runtime stage (line 30) installs `fastapi uvicorn` but NOT `prometheus-client`. The app imports `prometheus_client` at main.py:11-16 and will crash on startup. `compose/Dockerfile.python:30`, `py-api/pyproject.toml:9`
- [x] **`SystemTime::now().duration_since(UNIX_EPOCH).unwrap()` can panic** — Replaced with `domain::time::unix_timestamp_secs/millis()` using `unwrap_or(Duration::ZERO)`. `backend/auth/src/jwt.rs:57`, `backend/cache/src/rate_limit.rs:14-18`, `backend/domain/src/time.rs`

### HIGH

- [x] **Cookie auth mode unimplemented (always returns None)** — Cookie auth now fully implemented via `cookie_auth_prepare()` + `resolve_cookie_user()`. Session cookie parsing, CSRF validation, and Redis session lookup all working. `backend/auth/src/middleware.rs:79-141,165-238`
- [x] **DB error details swallowed** — All sqlx errors now logged via `tracing::error!()` before mapping to generic `ApiError::InternalError`. `backend/auth/src/routes.rs:136-139,157-159,271-273,441-443,627-629`
- [x] **Backoff increments silently discarded on Redis errors** — All `backoff_increment` calls now log failures via `tracing::warn!`. `backend/auth/src/routes.rs:279-281,290-292,298-300`
- [x] **Session destroy silently discarded** — Now logged via `tracing::warn!`. `backend/auth/src/routes.rs:372-375`
- [x] **XSS via `innerHTML` with unsanitized provider names** — Now uses `document.createElement("a")` with `a.textContent` instead of `innerHTML`. `frontend/src/components/AuthForm.astro:82-88`
- [x] **No password length upper bound** — Now validates `password.len() > 1024` with clear error message. `backend/auth/src/routes.rs:105-108`

### MEDIUM

- [ ] **Blacklist check fails open on Redis errors** — Still returns false on Redis error (`unwrap_or(None)` → `unwrap_or(false)`). Documented as intentional ("availability > revocation") but should be configurable. `backend/auth/src/middleware.rs:122-126`
- [x] **Hardcoded localhost OAuth redirect fallback** — Now returns `ApiError::InternalError("OAUTH_REDIRECT_URL not configured")` if env var not set, instead of defaulting to localhost. `backend/auth/src/routes.rs:549-551`
- [x] **`py-sidecar` status code defaults to 200 on parse failure** — Now defaults to 502 on parse failure. `backend/py-sidecar/src/lib.rs:335`
- [x] **Version triple-drift** — All three versions now synchronized at `0.11.2`. `py-api/app/main.py:144`, `py-api/pyproject.toml:3`, `VERSION`
- [x] **HMAC signature delimiter collision** — Now uses `json.dumps({...}, sort_keys=True)` for deterministic JSON serialization instead of pipe-delimited string. `py-api/app/main.py:121`, `backend/auth/src/middleware.rs:255-260`
- [ ] **`window.fetch` monkey-patch doesn't handle non-JSON bodies** — Partially improved: Request constructor preserves original body. But `performRefresh()` sends `body: JSON.stringify({})` and empty `catch {}` at line 166 still silently swallows some errors. `frontend/src/components/Layout.astro:154-172`
- [ ] **Duplicated health-check logic across client and server** — `isFullOutage()` and `getDiagnostics()` still defined in both `health.ts` (lines 25-31) and inline in `index.astro` (lines 258-275) with different signatures. `frontend/src/lib/health.ts`, `frontend/src/pages/index.astro:258-275`
- [x] **Auth response not validated before localStorage write** — Now validates `data.access_token` and `data.user` before redirect. Tokens use HttpOnly cookies set by backend, not localStorage. `frontend/src/components/AuthForm.astro:133-135`
- [x] **Retry timer stops on visibility change and never resumes** — Now handles `visibilitychange`: cancels retry on hidden, resets and re-fetches on visible. `frontend/src/pages/index.astro:401-408`
- [x] **`SIDECAR_SHARED_SECRET` missing from `.env`** — Now present in `.env:75` with dev value, and in `.env.example:98` as commented template. `.env:75`, `.env.example:96-98`

### LOW

- [x] **Non-null assertions on `getElementById` without null checks** — Now uses optional chaining (`if (guard) guard.style.display`). `frontend/src/pages/dashboard.astro:107-116`
- [x] **`window as any` type-safety bypass** — Now uses `interface CustomWindow extends Window` and `(window as CustomWindow)`. `frontend/src/components/Layout.astro:126-129`, `frontend/src/pages/dashboard.astro:126-128`
- [ ] **Empty catch blocks swallow errors** — Mix of patterns: `catch { return null; }`, `catch { /* documented */ }`, `catch { console.warn(...) }`, and `catch {}`. Not standardized. `frontend/src/components/AuthForm.astro:69`, `frontend/src/components/Layout.astro:121,148,166,177,184`
- [x] **Auth proxy doesn't forward cookies or trace headers** — Now forwards `content-type`, `authorization`, `cookie`, `x-trace-id`, and `x-forwarded-for`. `frontend/src/pages/api/auth/[...route].ts:14-22`
- [ ] **Module-level side effects on import** — `SHARED_SECRET` read at module level (main.py:19), `setup_logging()` now called via `@app.on_event("startup")` rather than at import time (improved but `SHARED_SECRET` is still module-level). `py-api/app/main.py:19,59-61`
- [ ] **Dockerfile health check hardcoded socket path** — Uses `os.getenv('PYTHON_SIDECAR_SOCKET','/tmp/sidecar/py-api.sock')` — default matches prod but still hardcoded. `compose/Dockerfile.python:44-45`
- [x] **Zombie `python-sidecar/` directory** — No longer exists. Deleted.

---

## Security

### CRITICAL

- [x] **Nginx runs as root in production** — Now runs as `user: "101"`. `compose/prod.yml:37`
- [x] **`.dockerignore` excluded from version control** — `.gitignore:24` now shows `docker-compose.override.yml`, not `.dockerignore`. `.dockerignore` is tracked in git.
- [x] **Prometheus `web.enable-lifecycle` exposed on host port** — `monitor.yml:27-30` no longer has `--web.enable-lifecycle` flag.
- [ ] **Dev compose exposes DB, Redis, RustFS ports without auth** — Ports now bound to `127.0.0.1` but Redis has no password in dev. `compose/dev.yml:36,72,100-102`

### HIGH

- [x] **Nginx config missing security headers** — Now includes X-Frame-Options DENY, X-Content-Type-Options nosniff, HSTS, CSP, Referrer-Policy, Permissions-Policy. `compose/nginx/nginx.conf:67-73`
- [ ] **Dev redis-exporter missing password while prod has it** — Dev uses `REDIS_PASSWORD: ${REDIS_PASSWORD:-}` (defaults empty) while prod uses `REDIS_PASSWORD: ${REDIS_PASSWORD}` (required). `compose/dev.yml:195`, `compose/prod.yml:284`
- [x] **OAuth `opencode.yml` triggers on any commenter** — Now checks for `OWNER`, `MEMBER`, or `COLLABORATOR` association. `.github/workflows/opencode.yml:11-14`
- [x] **Tokens stored in `localStorage`** — Now uses HttpOnly cookies set by backend via `Set-Cookie` headers. localStorage only used for theme preference. `backend/auth/src/routes.rs:181-199,335-353`

### MEDIUM

- [ ] **14 container images use `:latest` tags** — Down from 14 to 3: `rediscommander/redis-commander:latest`, `oliver006/redis_exporter:latest`, `prometheuscommunity/postgres-exporter:latest`. Multiple compose files.
- [ ] **All dev and monitor containers lack resource limits** — Dev services all have limits. Monitor: prometheus (lines 17-40) still has NO resource limits. `compose/monitor.yml`
- [ ] **Exporters expose metrics on host without auth** — `redis-exporter:9121`, `postgres-exporter:9187` publish ports with database internals. `compose/dev.yml:153-183`
- [x] **Alert rules entirely commented out** — `ServiceDown` and `HighLatency` rules are now active/uncommented. `compose/monitoring/alerts.yml`
- [x] **REDIS_URL missing password in `.env`** — Now includes password: `redis://:${REDIS_PASSWORD}@localhost:${REDIS_PORT}`. `.env:15`
- [ ] **`install.sh` uses `eval` with user input** — `eval "$*"` in `run()` and `run_in()` functions. `install.sh:31,43`
- [ ] **`scripts/config.sh` exports secrets to child processes** — `POSTGRES_PASSWORD` and `REDIS_PASSWORD` exported to all child processes. `scripts/config.sh:57-78`

### LOW

- [ ] **Monitor node-exporter mounts host `/proc`, `/sys`, `/`** — Still mounts without read-only restriction. `compose/monitor.yml:88-92`
- [ ] **Certbot has no resource limits** — `compose/prod.yml:339-348` (still missing)
- [x] **`SIDECAR_SHARED_SECRET` commented out in `.env.example`** — Now present as a template with `CHANGE_ME` placeholder. `.env.example:96-98`

---

## Performance

### HIGH

- [x] **New `reqwest::Client` per OAuth request** — `OAuthService` now stores `http_client: reqwest::Client` as a field, reused across requests. `backend/auth/src/oauth.rs:57,160-161`
- [x] **OAuthService reconstructed and secrets cloned on every request** — `AuthState` now holds `oauth: Arc<super::oauth::OAuthService>`, created once at startup. `backend/auth/src/routes.rs:25`

### MEDIUM

- [ ] **DB pool max connections only 5** — Configurable via `DB_MAX_CONNECTIONS` env var but default remains 5. `backend/api/src/lib.rs:47`, `.env:8`
- [x] **Unbounded streaming download** — Now checks `content-length` header and rejects downloads larger than 500MB; buffered download also has 100MB limit. `backend/storage/src/client.rs:131,163-176`
- [ ] **`format!()` allocations in hot paths** — Cookie header construction still uses `format!()` on every request. `backend/auth/src/routes.rs:184,193,338,347`
- [x] **`SIDECAR_SHARED_SECRET` env var read on every request** — Now read at module level (`main.py:19`) so it's cached at startup, not per-request. `py-api/app/main.py:19`
- [x] **No Python dependency caching in CI** — `ci.yml:112-117` now has `actions/cache@v4` step for `~/.cache/uv`. `.github/workflows/ci.yml:112-117`

### LOW

- [ ] **`String` where `&str` suffices in AuthConfig defaults** — `jwt_issuer` and other defaults still allocate. `backend/auth/src/lib.rs:62,72-73`
- [ ] **Large inline `<style>` blocks in Layout.astro** — ~273 lines of CSS still shipped inline. `frontend/src/components/Layout.astro:16-288`
- [ ] **Health retry causes full UI re-render flash** — On full outage, retry still resets all status dots to "loading". `frontend/src/pages/index.astro:303-306`
- [ ] **Dockerfile.python duplicate dependency installation** — Builder stage installs deps (line 21) that aren't copied to runtime; runtime re-installs `fastapi uvicorn` separately (line 30). Still wasteful. `compose/Dockerfile.python:21,30`

---

## Code Quality

### MEDIUM

- [ ] **Hardcoded `localhost:8001` in 4 source files** — Still used as fallback default in `health.ts:168`, `auth/[...route].ts:6`, `health.ts:8`, `astro.config.mjs:14`. All guarded by env var overrides.
- [ ] **Duplicated service ID lists** — `SERVICE_IDS` still defined in both `health.ts:1` and `index.astro:227`. Not shared constant.
- [x] **Inconsistent token key constants** — No more `TOKEN_KEY`/`USER_KEY`/`REFRESH_KEY` constants. Token management uses `document.cookie` directly with consistent `"access_token"`/`"refresh_token"` strings. `frontend/src/components/Layout.astro:122-124`
- [ ] **Pervasive `Record<string, unknown>` instead of typed interfaces** — Health check responses, auth responses, and user objects still untyped. `frontend/src/lib/health.ts`, `frontend/src/pages/index.astro:258-318`
- [ ] **`console.log` in production SSR code** — `jsonLog()` at `health.ts:21` calls `console.log` unconditionally (guarded by env check for dev/browser only). `frontend/src/lib/health.ts:21`
- [ ] **Missing function docstrings in Python** — `setup_logging()` and `JsonFormatter.format()` still lack docstrings. Other functions now have docstrings. `py-api/app/main.py:51,35`
- [ ] **Missing return type annotations on Python middlewares** — `trace_id_middleware` and `hmac_auth_middleware` still lack return type annotations. `py-api/app/main.py:67,91`
- [ ] **Inconsistent error handling patterns** — Mixed patterns: `catch { return null; }`, `catch { console.warn(...) }`, `catch {}`. Not standardized.
- [ ] **Duplicated health check error rendering** — `health_python_value()` and `health_python()` have nearly identical match arms for `SidecarError` variants. `backend/api/src/lib.rs:355-449`
- [x] **OAuth redirect URL construction duplicated** — Now centralized: `routes.rs` passes config-owned `redirect_url` to `OAuthService::get_redirect_url()`. No more hardcoded localhost fallback. `backend/auth/src/routes.rs:544-553`
- [ ] **Inconsistent naming conventions** — `AuthMode::Both` still used (vs `Hybrid`/`CookieAndBearer`). `backend/auth/src/lib.rs:43`
- [ ] **Missing `__init__.py` and `conftest.py` in `py-api/tests/`** — No shared test fixtures. `autouse` fixture still in individual test files. `py-api/tests/`
- [ ] **Test runners split across Bun and Vitest** — Both `bun test` and `vitest` still referenced in CI and config.

### LOW

- [x] **`let _ =` discards errors in production code** — All `let _ =` patterns replaced with proper `if let Err(e)` + `tracing::warn!` logging. `backend/auth/src/routes.rs`, `backend/cache/src/rate_limit.rs`
- [ ] **`unwrap()` in non-test Rust code** — `main.rs:25,28` still has `unwrap()` for address parsing and `TcpListener::bind`. `backend/api/src/main.rs:25,28`, production code elsewhere
- [ ] **`unsafe { std::env::set_var() }` in tests** — Race condition risk remains in multithreaded test context. `backend/auth/src/lib.rs:128-158`, `backend/storage/src/lib.rs:147-189`
- [ ] **`as any` type casts in test files** — Double-cast pattern (`as unknown as typeof fetch`) still present in multiple frontend test files.
- [ ] **Inconsistent shebangs** — `status.sh` uses `#!/usr/bin/env bash` while others use `#!/bin/bash`. `scripts/status.sh:1`
- [ ] **`py-api` vs `python-sidecar` naming inconsistency** — Env vars still reference `PYTHON_SIDECAR_*`, directory name `python-sidecar/` was deleted but env var naming inconsistency remains. Root-level.

---

## Documentation

### HIGH

- [x] **INFRASTRUCTURE.md references non-existent `static.conf`** — Now correctly references `compose/nginx/nginx.conf`. `docs/INFRASTRUCTURE.md:712-714`
- [x] **Nginx security headers documented but not implemented** — Both documented in INFRASTRUCTURE.md and now implemented in `nginx.conf:67-73`. `docs/INFRASTRUCTURE.md:708`, `compose/nginx/nginx.conf`
- [x] **CI.md claims Docker push step that doesn't exist** — CI.md no longer claims a Docker push step. `docs/CI.md:29-31`
- [x] **CI.md lists wrong required secrets** — CI.md secrets list now matches actual CI workflow. `docs/CI.md:37-44`

### MEDIUM

- [ ] **`performance-budget.md` references `bombardier` but `bench.sh` uses `ab`** — Documentation describes wrong benchmarking tool. `docs/performance-budget.md:7-11`, `scripts/bench.sh:20`
- [x] **MONITORING.md alert examples use wrong metric names** — Now matches: both use `http_request_duration_seconds_bucket` and correct threshold. `docs/MONITORING.md:172-173`, `compose/monitoring/alerts.yml:23-24`
- [x] **DEPLOY.md references `.deploy-state/lock` that doesn't exist** — Now explicitly documents that no automated deploy lock exists. `docs/DEPLOY.md:46`
- [ ] **Socket path naming differs between dev and prod with no migration doc** — Dev uses `/tmp/fullstackhex-python.sock`, prod uses `/tmp/sidecar/py-api.sock`. `docs/ARCHITECTURE.md:106`, `compose/prod.yml:79`
- [ ] **`.env` vs `.env.example` inconsistencies** — `.env` has keys not in `.env.example` (DB_MAX_CONNECTIONS, REDIS_KEY_PREFIX, REDIS_POOL_SIZE, SIDECAR_SHARED_SECRET, RUSTFS_*). `.env.example` has OAuth sections not in `.env`.
- [ ] **INFRASTRUCTURE.md embedded compose section is outdated** — Doesn't include exporters, ADMINER_PORT, REDIS_COMMANDER_PORT that are in actual dev.yml. `docs/INFRASTRUCTURE.md:236-386`
- [ ] **No disaster recovery or scaling documentation** — No runbooks for data loss, container failure, or horizontal scaling.
- [ ] **No secrets rotation guide** — No documentation on rotating `JWT_SECRET`, `DATABASE_URL` passwords, or `RUSTFS` keys.
- [ ] **No TLS renewal automation doc** — Certbot container exists but no documentation on renewal flow or monitoring.

### LOW

- [ ] **`DESIGN.md` about documentation style, not system design** — Confusingly named for a system architecture document. `docs/DESIGN.md`
- [ ] **Missing doc sections on `AuthUser` fields** — `user_id`, `email`, `name`, `provider` now have doc comments. `jti` and `session_id` also documented. `backend/auth/src/middleware.rs:19-30`
- [ ] **Missing `# Panics` / `# Errors` doc sections** — Public functions like `backoff_check`, `backoff_increment` now have doc comments but still lack `# Errors` sections. `backend/cache/src/rate_limit.rs`
- [ ] **MONITORING.md is vague on Grafana/Prometheus versions** — Says "Prometheus 3.x + Grafana" with no specific version. `docs/MONITORING.md:24`

---

## Infrastructure

### HIGH

- [x] **Add security headers to nginx config** — Implemented: X-Frame-Options DENY, X-Content-Type-Options nosniff, HSTS, CSP, Referrer-Policy, Permissions-Policy. `compose/nginx/nginx.conf:67-73`
- [ ] **Pin all `:latest` image tags to specific versions** — 3 images still use `:latest`: `rediscommander/redis-commander:latest`, `oliver006/redis_exporter:latest`, `prometheuscommunity/postgres-exporter:latest`. Multiple compose files.
- [x] **Add Python dependency caching to CI** — `actions/cache@v4` step added for `~/.cache/uv`. `.github/workflows/ci.yml:112-117`
- [x] **Fix `REDIS_URL` in `.env` to include password** — Now `redis://:${REDIS_PASSWORD}@localhost:${REDIS_PORT}`. `.env:15`

### MEDIUM

- [x] **Add `set -euo pipefail` to `common.sh`, `test.sh`, `logs.sh`** — All three now have `set -euo pipefail`. `scripts/common.sh:2`, `scripts/test.sh:2`, `scripts/logs.sh:2`
- [ ] **Add resource limits to all dev and monitor containers** — Dev services all have limits. Prometheus in `monitor.yml` still has NO resource limits. `compose/monitor.yml`
- [x] **Add health checks to adminer, redis-commander, redis-exporter, postgres-exporter, alertmanager** — All now have health checks. `compose/dev.yml`, `compose/monitor.yml`
- [x] **Add Docker ecosystem to dependabot** — Now includes Docker ecosystem pointing to `/compose`. `.github/dependabot.yml:53-58`
- [x] **Remove `.dockerignore` from `.gitignore`** — `.gitignore:24` no longer excludes `.dockerignore`. `.dockerignore` is tracked in git.
- [ ] **Add backup scripts** — No automated backup/restore scripts exist. `scripts/`
- [ ] **Add `conftest.py` to `py-api/tests/`** — No shared test fixtures. `py-api/tests/`
- [x] **Run nginx as non-root user** — Now runs as `user: "101"`. `compose/prod.yml:37`
- [x] **Remove or restrict Prometheus `web.enable-lifecycle`** — Flag removed from prometheus command. `compose/monitor.yml:27-30`
- [ ] **Configure alertmanager receivers** — Default receiver exists but only logs to stdout. Slack/PagerDuty configs are commented out. `compose/monitoring/alertmanager.yml:23-29`
- [x] **Uncomment alert rules** — `ServiceDown` and `HighLatency` rules are now active. `compose/monitoring/alerts.yml`

### LOW

- [ ] **Add resource limits to certbot container** — Still missing. `compose/prod.yml:339-348`
- [ ] **Use read-only mounts for node-exporter** — Mounts `/proc`, `/sys`, `/` without read-only flag. `compose/monitor.yml:88-92`
- [ ] **Add `depends_on` with health checks for backend services** — Not added for DB/Redis in prod compose. `compose/prod.yml`
- [x] **Delete zombie `python-sidecar/` directory** — Directory no longer exists.