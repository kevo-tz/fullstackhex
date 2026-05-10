# TODOS

Deep-dive audit findings across Rust backend, Astro frontend, Python API, and infra/docs.
Organized by category with priority tags. Verified against source code on 2026-05-10.

---

## Bugs

### CRITICAL

- [ ] **CSRF bypass when cookie+header both absent** — `validate_csrf_token("", "")` returns true (csrf.rs:60-61). In cookie auth mode, if neither `x-csrf-token` header nor `csrf_token` cookie is present, `unwrap_or("")` feeds empty strings to validation, which passes. Latent vulnerability: `extract_cookie` always returns None (middleware.rs:202-207), so this isn't exploitable yet, but will be when cookie auth is implemented. `backend/auth/src/middleware.rs:179-199`, `backend/auth/src/csrf.rs:18-27`

- [ ] **`prometheus-client` missing from Dockerfile** — Dockerfile.python only installs `fastapi` and `uvicorn` (lines 21, 30) but not the declared `prometheus-client>=0.21,<0.26` dependency. Service crashes on startup with `ImportError`. `compose/Dockerfile.python:21,30`, `py-api/pyproject.toml:9`

- [ ] **`SystemTime::now().duration_since(UNIX_EPOCH).unwrap()` can panic** — If system clock is before Unix epoch (NTP misconfiguration, VM clock issues), the server panics and dies. Affects JWT token creation, rate-limit timestamp computation. `backend/auth/src/jwt.rs:58-60`, `backend/cache/src/rate_limit.rs:40-42,94-96`

### HIGH

- [ ] **Cookie auth mode unimplemented (always returns None)** — `extract_cookie()` reads the session cookie and validates CSRF, then unconditionally logs a warning and returns None. Users with `AUTH_MODE=cookie` can never authenticate. `backend/auth/src/middleware.rs:202-207`

- [ ] **DB error details swallowed** — All sqlx errors at `routes.rs:135,153,244,363,536,539` are mapped to generic `ApiError::InternalError("Internal server error")`, discarding diagnostic information. Makes production debugging nearly impossible. At minimum, log the original error via `tracing::error!()`. `backend/auth/src/routes.rs`

- [ ] **Backoff increments silently discarded on Redis errors** — `let _ = state.redis.backoff_increment(&ip, "login").await;` loses rate-limit tracking. Attackers can brute-force during Redis downtime. `backend/auth/src/routes.rs:249,258,264`

- [ ] **Session destroy silently discarded** — `let _ = state.redis.session_destroy(session_id).await;` fails silently, leaking sessions. `backend/auth/src/routes.rs:317`

- [ ] **XSS via `innerHTML` with unsanitized provider names** — OAuth provider names from `/api/auth/providers` response are injected directly into DOM via `innerHTML` without sanitization. A compromised backend could inject arbitrary HTML/JS. `frontend/src/components/AuthForm.astro:81-83`

- [ ] **No password length upper bound** — `validate_registration` checks minimum length (8 chars) but has no maximum. Argon2 hashing on multi-gigabyte passwords causes CPU exhaustion. `backend/auth/src/routes.rs:100-110`, `backend/auth/src/password.rs:9-19`

### MEDIUM

- [ ] **Blacklist check fails open on Redis errors** — When Redis is unreachable, `cache_get` returns `Err` → `unwrap_or(None)` → `is_blacklisted.unwrap_or(false)` = false, accepting blacklisted tokens. Currently documented as intentional ("availability > revocation") but should be configurable. `backend/auth/src/middleware.rs:92-96`

- [ ] **Hardcoded localhost OAuth redirect fallback** — If `OAUTH_REDIRECT_URL` env var is not set, production requests redirect to `http://localhost:8001/auth/oauth/{provider}/callback`. Should fail with a configured error instead. `backend/auth/src/routes.rs:454`

- [ ] **`py-sidecar` status code defaults to 200 on parse failure** — If HTTP status line can't be parsed from the sidecar response, defaults to 200 (success), masking server errors. `backend/py-sidecar/src/lib.rs:335`

- [ ] **Version triple-drift** — Health endpoint returns `0.7.0`, pyproject.toml says `0.1.0`, monorepo VERSION file says `0.11.1`. All three should be synchronized. `py-api/app/main.py:140`, `py-api/pyproject.toml:3`, `VERSION`

- [ ] **HMAC signature delimiter collision** — Pipe `|` delimiter in `f"{user_id}|{email}|{name}"` allows ambiguity if header values contain `|`. Crafted headers could produce the same HMAC payload with different field assignments. `py-api/app/main.py:118`

- [ ] **`window.fetch` monkey-patch doesn't handle non-JSON bodies** — FormData, Blob, ArrayBuffer, or URLSearchParams bodies on 401 responses silently fail retry (line 443: `typeof body !== "string"` returns original 401 response). File uploads and form submissions that get 401s won't trigger token refresh redirect. `frontend/src/components/Layout.astro:434-454`

- [ ] **Duplicated health-check logic across client and server** — `isFullOutage()` and `getDiagnostics()` implemented both in `health.ts` (lines 5-37) and inline in `index.astro` (lines 258-275). Tests even have "KEEP IN SYNC" comments. Any server-side changes won't be reflected on client without manual sync. `frontend/src/lib/health.ts`, `frontend/src/pages/index.astro:258-275`

- [ ] **Auth response not validated before localStorage write** — After `res.ok`, code assumes `data.access_token`, `data.refresh_token`, `data.user` exist. If API response format changes, `undefined` gets written to localStorage. `frontend/src/components/AuthForm.astro:127-131`

- [ ] **Retry timer stops on visibility change and never resumes** — `visibilitychange` listener calls `cancelRetry()` but never restarts retries when tab becomes visible again. During full outages, polls stop permanently until manual refresh. Also `retryDelay` is never reset on successful fetch if `startRetry` isn't called. `frontend/src/pages/index.astro:280-307,401-408`

- [ ] **`SIDECAR_SHARED_SECRET` missing from `.env`** — If not set, Python sidecar rejects ALL requests (fail-closed). `.env.example` has it commented out (line 98). `.env` doesn't include it at all, breaking dev sidecar auth. `.env:30-32`, `.env.example:96-98`

### LOW

- [ ] **Non-null assertions on `getElementById` without null checks** — Dashboard uses `!` assertions (`document.getElementById("auth-guard")!` etc.) that throw at runtime if elements are missing. Should use optional chaining. `frontend/src/pages/dashboard.astro:107-116`

- [ ] **`window as any` type-safety bypass** — `logout` function attached to window with `(window as any).logout = logout`, defeating TypeScript. `frontend/src/components/Layout.astro:401`, `frontend/src/pages/dashboard.astro:121`

- [ ] **Empty catch blocks swallow errors** — Multiple `catch {}` blocks silently discard errors in AuthForm (line 69), Layout (lines 377, 459, 466), and health.ts (lines 99, 135). `frontend/src/components/AuthForm.astro:69`, `frontend/src/components/Layout.astro:377,459,466`

- [ ] **Auth proxy doesn't forward cookies or trace headers** — Only `content-type` and `authorization` are forwarded; cookies, trace IDs (`x-trace-id`), and custom headers dropped. `frontend/src/pages/api/auth/[...route].ts:12-16`

- [ ] **Module-level side effects on import** — `setup_logging()` called at module level (line 60), clears root logger handlers (`root.handlers.clear()`), making isolated testing impossible. `py-api/app/main.py:55-61`

- [ ] **Dockerfile health check hardcoded socket path** — Health check uses `/tmp/sidecar/py-api.sock` but env var `PYTHON_SIDECAR_SOCKET` defaults to `/tmp/fullstackhex-python.sock`. Custom socket paths will cause health check to always fail. `compose/Dockerfile.python:44-45`

- [ ] **Zombie `python-sidecar/` directory** — Leftover from v0.10.1 rename. Contains only `.pytest_cache/`, `.ruff_cache/`, `.venv/` — no source files, not tracked by git. Should be deleted.

---

## Security

### CRITICAL

- [ ] **Nginx runs as root in production** — compose/prod.yml nginx service has no `user` directive or `USER` instruction. Compromised nginx process has root in container. `compose/prod.yml:35-61`

- [ ] **`.dockerignore` excluded from version control** — `.gitignore` line 24 lists `.dockerignore`. If deleted locally, Docker builds include `.git`, `node_modules`, and secrets. `compose/Dockerfile.*` files rely on `.dockerignore` to exclude these. `.gitignore:24`

- [ ] **Prometheus `web.enable-lifecycle` exposed on host port** — Allows remote config reloads. Combined with unauthenticated port 9090, anyone can modify Prometheus configuration. `compose/monitor.yml:30`

- [ ] **Dev compose exposes DB, Redis, RustFS ports without auth** — PostgreSQL (5432), Redis (6379), and RustFS (9000, 9001) published to host. `compose/dev.yml:36,67,91-92`

### HIGH

- [ ] **Nginx config missing security headers** — No `X-Frame-Options`, `X-Content-Type-Options`, `X-XSS-Protection`, `Content-Security-Policy`, or `Strict-Transport-Security` headers in HTTPS server block. INFRASTRUCTURE.md (line 708) claims these exist. `compose/nginx/nginx.conf:56-119`

- [ ] **Dev redis-exporter missing password while prod has it** — Dev `redis-exporter` connects without `REDIS_PASSWORD` (compose/dev.yml:159), while prod correctly passes it (compose/prod.yml:282-283). Dev metrics collection will fail silently if Redis auth is enabled. `compose/dev.yml:152-164`

- [ ] **OAuth `opencode.yml` triggers on any commenter** — Any user can trigger `/oc` or `/opencode` with `id-token: write` permissions. Should restrict to repo collaborators. `.github/workflows/opencode.yml:11-15,18`

- [ ] **Tokens stored in `localStorage`** — Vulnerable to XSS exfiltration. Combined with the innerHTML XSS vulnerability (BUG-HIGH-5), tokens are directly accessible to any injected script. Should use httpOnly cookies. `frontend/src/components/AuthForm.astro:128-130`

### MEDIUM

- [ ] **14 container images use `:latest` tags** — Including production `rustfs/rustfs:latest` (compose/prod.yml:237). No reproducibility or rollback guarantee. Pin to specific versions. Multiple compose files.

- [ ] **All dev and monitor containers lack resource limits** — No `deploy.resources.limits` on any dev.yml service or monitor.yml services (prometheus, grafana, alertmanager). Unbounded resource consumption possible. `compose/dev.yml`, `compose/monitor.yml`

- [ ] **Exporters expose metrics on host without auth** — `redis-exporter:9121`, `postgres-exporter:9187` publish ports with database internals. `compose/dev.yml:153-183`

- [ ] **Alert rules entirely commented out** — All rules in `alerts.yml` are commented out. Alertmanager has no receivers configured. No active alerting. `compose/monitoring/alerts.yml:8-30`, `compose/monitoring/alertmanager.yml:23-30`

- [ ] **REDIS_URL missing password in `.env`** — `REDIS_URL=redis://localhost:${REDIS_PORT}` without password. Production should use `redis://:${REDIS_PASSWORD}@localhost:${REDIS_PORT}`. `.env:14`

- [ ] **`install.sh` uses `eval` with user input** — `eval "$*"` and `eval "$*"` in `run()` and `run_in()` functions. `install.sh:31,43`

- [ ] **`scripts/config.sh` exports secrets to child processes** — `POSTGRES_PASSWORD` and `REDIS_PASSWORD` exported to all child processes. `scripts/config.sh:57-78`

### LOW

- [ ] **Monitor node-exporter mounts host `/proc`, `/sys`, `/`** — Security risk if container is compromised. Use read-only mounts or restrict paths. `compose/monitor.yml:88-92`

- [ ] **Certbot has no resource limits** — `compose/prod.yml:339-348`

- [ ] **`SIDECAR_SHARED_SECRET` commented out in `.env.example`** — Should be required in dev, not optional, to prevent sidecar auth bypass. `.env.example:96-98`

---

## Performance

### HIGH

- [ ] **New `reqwest::Client` per OAuth request** — `OAuthService::exchange_code()` creates a new `reqwest::Client` on every call (oauth.rs:142-144), and `fetch_google_user_info()` (line 174) and `fetch_github_user_info()` (line 202) each create another. `reqwest::Client` is designed to be reused for connection pooling. Store in `OAuthService` or `AuthState`. `backend/auth/src/oauth.rs:142-144,174,202`

- [ ] **OAuthService reconstructed and secrets cloned on every request** — Client IDs and secrets cloned on every OAuth flow invocation via `OAuthService::new()` with `.clone()`. Should be created once in `AuthState`. `backend/auth/src/routes.rs:437-440,504-507`

### MEDIUM

- [ ] **DB pool max connections only 5** — May be insufficient under concurrent auth + storage load. Should be configurable via `DB_MAX_CONNECTIONS` env var. `backend/api/src/lib.rs:43`

- [ ] **Unbounded streaming download** — `download_streaming()` has no size limit (unlike `download()` which caps at 100MB). Malicious S3 endpoint could stream infinite data, exhausting proxy memory. Add content-length check. `backend/storage/src/client.rs:141-163`

- [ ] **`format!()` allocations in hot paths** — Rate-limit keys, metric labels, and hex encoding allocate `String` on every request. Pre-compute or use `&'static str` labels for metrics. `backend/auth/src/routes.rs:121-122,220-223`, `backend/cache/src/rate_limit.rs:128`, `backend/api/src/metrics.rs:92-93`

- [ ] **`SIDECAR_SHARED_SECRET` env var read on every request** — Should be cached at startup via FastAPI lifecycle event or `lru_cache`. `py-api/app/main.py:94`

- [ ] **No Python dependency caching in CI** — `uv sync --all-extras` runs from scratch every time. Add `actions/cache` for `~/.cache/uv`. `.github/workflows/ci.yml:94-122`

### LOW

- [ ] **`String` where `&str` suffices in AuthConfig defaults** — `jwt_issuer` and other defaults allocate on every config load. `backend/auth/src/lib.rs:62,72-73`

- [ ] **Large inline `<style>` blocks in Layout.astro** — ~273 lines of CSS shipped on every page request. Extract to cached stylesheet. `frontend/src/components/Layout.astro:16-288`

- [ ] **Health retry causes full UI re-render flash** — On full outage, every retry resets all status dots to "loading". `frontend/src/pages/index.astro:303-306`

- [ ] **Dockerfile.python duplicate dependency installation** — Dependencies installed in builder stage (line 21) AND in runtime stage (lines 29-30). Builder stage install is wasted since nothing is copied from it. `compose/Dockerfile.python:21,29-30`

---

## Code Quality

### MEDIUM

- [ ] **Hardcoded `localhost:8001` in 4 source files** — Should be single shared constant or env var. `frontend/src/pages/api/health.ts:8`, `frontend/src/pages/api/auth/[...route].ts:6`, `frontend/src/lib/health.ts:149`, `frontend/astro.config.mjs:14`

- [ ] **Duplicated service ID lists** — `["rust", "db", "redis", "storage", "python", "auth"]` defined in 3 separate locations. Should be exported constant. `frontend/src/pages/index.astro:227`, `frontend/src/lib/health.ts:6,17`

- [ ] **Inconsistent token key constants** — `TOKEN_KEY`, `USER_KEY`, `REFRESH_KEY` defined separately in Layout.astro and dashboard.astro; AuthForm.astro uses hardcoded strings instead of constants. `frontend/src/components/Layout.astro:331-333`, `frontend/src/pages/dashboard.astro:102-103`, `frontend/src/components/AuthForm.astro:128-130`

- [ ] **Pervasive `Record<string, unknown>` instead of typed interfaces** — Health check responses, auth responses, and user objects all untyped. Typos in property access won't be caught. `frontend/src/lib/health.ts`, `frontend/src/pages/index.astro:258-318`, `frontend/src/components/Layout.astro:340-380`

- [ ] **`console.log` in production SSR code** — `jsonLog` function calls `console.log` unconditionally on every health check request. Should use proper logger or guard behind `import.meta.env.DEV`. `frontend/src/lib/health.ts:2`

- [ ] **Missing function docstrings in Python** — 6 public functions in `app/main.py` lack docstrings. `py-api/app/main.py:35,50,65,88,136,144`

- [ ] **Missing return type annotations on Python middlewares** — `trace_id_middleware` and `hmac_auth_middleware` lack return type annotations. `py-api/app/main.py:65,88`

- [ ] **Inconsistent error handling patterns** — Mix of empty `catch {}`, `catch` with `console.warn`, and structured `catch` with error logging across frontend. Standardize pattern.

- [ ] **Duplicated health check error rendering** — `health_python_value()` and `health_python()` handler have nearly identical match arms. `backend/api/src/lib.rs:343-440`

- [ ] **OAuth redirect URL construction duplicated** — Appears in routes.rs:454 and test assertions in oauth.rs:335,355. `backend/auth/src/routes.rs:454`, `backend/auth/src/oauth.rs:335,355`

- [ ] **Inconsistent naming conventions** — `AuthMode::Both` is ambiguous (vs `Hybrid`/`CookieAndBearer`). `DbStatus` variants use different conventions from `ApiError`. Backend-wide.

- [ ] **Missing `__init__.py` and `conftest.py` in `py-api/tests/`** — No shared test fixtures. `autouse` fixture defined in test file instead of conftest. `py-api/tests/test_hmac_middleware.py:45-48`

- [ ] **Test runners split across Bun and Vitest** — Overlapping test coverage in `frontend/tests/` with different runners increases maintenance burden.

- [ ] **14 container images use `:latest` tags** — Pin all image versions for reproducibility. `compose/dev.yml`, `compose/prod.yml`, `compose/monitor.yml`

### LOW

- [ ] **`let _ =` discards errors in production code** — Multiple locations silently discard Redis/command results without logging. `backend/auth/src/routes.rs:249,258,264,317`, `backend/cache/src/rate_limit.rs:156`

- [ ] **`unwrap()` in non-test Rust code** — 6 locations use `unwrap()` in production code, some risky (e.g., `TcpListener::bind(addr).await.unwrap()` panics if port in use). `backend/api/src/main.rs:25,28,44`, `backend/auth/src/jwt.rs:59`, `backend/cache/src/rate_limit.rs:41,95`

- [ ] **`unsafe { std::env::set_var() }` in tests** — Race condition risk in multithreaded test context. `backend/auth/src/lib.rs:129-158`, `backend/storage/src/lib.rs:147-189`, `backend/py-sidecar/src/lib.rs:416-449`

- [ ] **`as any` type casts in test files** — Double-cast pattern `as unknown as typeof fetch` indicates weak test types. Multiple frontend test files.

- [ ] **Inconsistent shebangs** — `status.sh` uses `#!/usr/bin/env bash` while others use `#!/bin/bash`. `scripts/status.sh:1`

- [ ] **`py-api` vs `python-sidecar` naming inconsistency** — Env vars still reference `PYTHON_SIDECAR_*`, directory name `python-sidecar/` is a leftover. Root-level.

---

## Documentation

### HIGH

- [ ] **INFRASTRUCTURE.md references non-existent `static.conf`** — Says "Minimal config for serving an Astro static build" referencing `compose/nginx/static.conf`, which doesn't exist. Only `canary.conf`, `upstream.conf.template`, and `nginx.conf` exist. `docs/INFRASTRUCTURE.md:712-714`

- [ ] **Nginx security headers documented but not implemented** — INFRASTRUCTURE.md line 708 claims "HSTS, X-Content-Type-Options, X-Frame-Options headers" exist in nginx config, but `nginx.conf` has none of these. `docs/INFRASTRUCTURE.md:708`, `compose/nginx/nginx.conf`

- [ ] **CI.md claims Docker push step that doesn't exist** — Says "Build Docker Images (main branch only)" and "Pushes to container registry" but the actual CI workflow has no build/push Docker images step. `docs/CI.md:29-31`

- [ ] **CI.md lists wrong required secrets** — Claims `DOCKER_USERNAME` and `DOCKER_PASSWORD` are needed, but no Docker push step exists. Claims `REDIS_PASSWORD` is needed for e2e but e2e uses `redis://localhost:6379` without password. `docs/CI.md:37-44`

### MEDIUM

- [ ] **`performance-budget.md` references `bombardier` but `bench.sh` uses `ab`** — Documentation describes wrong benchmarking tool. `docs/performance-budget.md:7-11`, `scripts/bench.sh:20`

- [ ] **MONITORING.md alert examples use wrong metric names** — Examples use `http_request_duration_ms_bucket` (line 172) and threshold `> 100` (meaning 100ms), but the actual metric is `http_request_duration_seconds_bucket` with threshold `> 0.1` (0.1 seconds). `docs/MONITORING.md:172-173` vs `compose/monitoring/alerts.yml:23-24`

- [ ] **DEPLOY.md references `.deploy-state/lock` that doesn't exist** — No script creates this file or directory. `docs/DEPLOY.md:46`

- [ ] **Socket path naming differs between dev and prod with no migration doc** — Dev uses `/tmp/fullstackhex-python.sock`, prod uses `/tmp/sidecar/py-api.sock`. `docs/ARCHITECTURE.md:106`, `compose/prod.yml:79`

- [ ] **`.env` vs `.env.example` inconsistencies** — `REDIS_KEY_PREFIX`, `REDIS_POOL_SIZE`, monitoring port vars missing from `.env`. `SIDECAR_SHARED_SECRET` missing from `.env`. Multiple vars out of sync.

- [ ] **INFRASTRUCTURE.md embedded compose section is outdated** — Doesn't include exporters, ADMINER_PORT, REDIS_COMMANDER_PORT that are in actual dev.yml. `docs/INFRASTRUCTURE.md:236-386`

- [ ] **No disaster recovery or scaling documentation** — No runbooks for data loss, container failure, or horizontal scaling.

- [ ] **No secrets rotation guide** — No documentation on rotating `JWT_SECRET`, `DATABASE_URL` passwords, or `RUSTFS` keys.

- [ ] **No TLS renewal automation doc** — Certbot container exists but no documentation on renewal flow or monitoring.

### LOW

- [ ] **`DESIGN.md` about documentation style, not system design** — Confusingly named for a system architecture document. `docs/DESIGN.md`

- [ ] **Missing doc sections on `AuthUser` fields** — `user_id`, `email`, `name`, `provider` lack doc comments. `backend/auth/src/middleware.rs:19-22`

- [ ] **Missing `# Panics` / `# Errors` doc sections** — Public functions like `backoff_check`, `backoff_increment` lack these. `backend/cache/src/rate_limit.rs`

- [ ] **MONITORING.md is vague on Grafana/Prometheus versions** — Says "Prometheus 3.x + Grafana" with no specific version. `docs/MONITORING.md:24`

---

## Infrastructure

### HIGH

- [ ] **Add security headers to nginx config** — Missing `X-Frame-Options`, `X-Content-Type-Options`, `X-XSS-Protection`, `Content-Security-Policy`, `Strict-Transport-Security`. `compose/nginx/nginx.conf:56-119`

- [ ] **Pin all `:latest` image tags to specific versions** — 14 images across dev/prod/monitor use `:latest`. Pin for reproducibility and rollback. Multiple compose files.

- [ ] **Add Python dependency caching to CI** — `uv sync` runs from scratch each time. Add `actions/cache` step for `~/.cache/uv`. `.github/workflows/ci.yml:94-122`

- [ ] **Fix `REDIS_URL` in `.env` to include password** — Currently `redis://localhost:${REDIS_PORT}` without password. `.env:14`

### MEDIUM

- [ ] **Add `set -euo pipefail` to `common.sh`, `test.sh`, `logs.sh`** — Other scripts use it, these don't. `scripts/common.sh`, `scripts/test.sh`, `scripts/logs.sh`

- [ ] **Add resource limits to all dev and monitor containers** — No `deploy.resources.limits` on dev services, prometheus, grafana, or alertmanager. `compose/dev.yml`, `compose/monitor.yml`

- [ ] **Add health checks to adminer, redis-commander, redis-exporter, postgres-exporter, alertmanager** — Missing from compose files.

- [ ] **Add Docker ecosystem to dependabot** — Current config covers GitHub Actions, Cargo, pip, npm but not Docker image tags. `.github/dependabot.yml`

- [ ] **Remove `.dockerignore` from `.gitignore`** — `.dockerignore` should be tracked in version control. `.gitignore:24`

- [ ] **Add backup scripts** — No automated backup/restore. INFRASTRUCTURE.md documents manual `pg_dump` but no scripts exist. `scripts/`

- [ ] **Add `conftest.py` to `py-api/tests/`** — Move `autouse` fixture to shared location. `py-api/tests/`

- [ ] **Run nginx as non-root user** — Add `user nginx;` directive or `USER` in compose. `compose/prod.yml:35-61`

- [ ] **Remove or restrict Prometheus `web.enable-lifecycle`** — Or restrict access via network policy. `compose/monitor.yml:30`

- [ ] **Configure alertmanager receivers** — No notification integrations active. `compose/monitoring/alertmanager.yml:23-30`

- [ ] **Uncomment alert rules** — All rules in `alerts.yml` are commented out. `compose/monitoring/alerts.yml:8-30`

### LOW

- [ ] **Add resource limits to certbot container** — `compose/prod.yml:339-348`

- [ ] **Use read-only mounts for node-exporter** — Mount `/proc`, `/sys`, `/` as `ro`. `compose/monitor.yml:88-92`

- [ ] **Add `depends_on` with health checks for backend services** — Ensure DB and Redis are healthy before backend starts. `compose/prod.yml`

- [ ] **Delete zombie `python-sidecar/` directory** — Leftover from v0.10.1 rename, only contains caches.