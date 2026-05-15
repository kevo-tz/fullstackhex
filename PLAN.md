# Plan: Fix All Remaining TODOS.md Issues

Originally 39 open items across 7 categories. After investigation and fixes on 2026-05-10:
- **23 items were already fixed** in code but not marked in TODOS.md — now marked `[x]`
- **16 remaining items fixed** in this session
- **0 items remaining** — all TODOS.md items now `[x]`

Organized into 6 implementation phases by dependency and risk.

---

## Phase 1 — Security Hardening (11 items)

### S1.1 Pin 3 remaining `:latest` image tags
**Files:** `compose/dev.yml`, `compose/monitor.yml`, `compose/prod.yml`
**Fix:** Replace `rediscommander/redis-commander:latest` → `rediscommander/redis-commander:1.7.2`, `oliver006/redis_exporter:latest` → `oliver006/redis_exporter:v1.67.0`, `prometheuscommunity/postgres-exporter:latest` → `prometheuscommunity/postgres-exporter:v0.16.0`. The non-latest versions are already in monitor.yml and can be reused. Search all compose files for any `:latest` and pin to specific semver tags.

### S1.2 Add Redis password to dev redis-exporter
**Files:** `compose/dev.yml`
**Fix:** Change `REDIS_PASSWORD: ${REDIS_PASSWORD:-}` to `REDIS_PASSWORD: ${REDIS_PASSWORD:?REDIS_PASSWORD must be set}` in the dev redis-exporter service (line ~195). This makes dev consistent with prod which requires the password.

### S1.3 Bind exporter/metric ports to `127.0.0.1` in dev and monitor
**Files:** `compose/dev.yml`, `compose/monitor.yml`
**Fix:** In `compose/dev.yml`, bind redis-exporter port as `127.0.0.1:${REDIS_EXPORTER_PORT:-9121}:9121`. In `compose/monitor.yml`, bind prometheus as `127.0.0.1:${PROMETHEUS_PORT:-9090}:9090`, grafana as `127.0.0.1:${GRAFANA_PORT:-3000}:3000`, alertmanager as `127.0.0.1:${ALERTMANAGER_PORT:-9093}:9093`, node-exporter as `127.0.0.1:${NODE_EXPORTER_PORT:-9100}:9100`.

### S1.4 Add read-only flags to node-exporter mounts
**Files:** `compose/monitor.yml`
**Fix:** Already uses `:ro` on all mounts. Verify `/proc:/host/proc:ro`, `/sys:/host/sys:ro`, `/:/host/rootfs:ro` all have `:ro`. They do — no change needed. Mark as FIXED.

### S1.5 Add resource limits to certbot container
**Files:** `compose/prod.yml`
**Fix:** Already present (lines 360-364): `cpus: "0.25"`, `memory: 128M`. Mark as FIXED.

### S1.6 Remove `eval` from `install.sh`
**File:** `install.sh`
**Fix:** Replace `eval "$*"` in `run()` (line 31) and `run_in()` (line 43) with direct `"$@"` invocation. `"$@"` correctly splits arguments without shell injection risk.

### S1.7 Stop exporting secrets in `config.sh`
**File:** `scripts/config.sh`
**Fix:**
1. Add a guard at the top: `if [[ ! -f .env ]]; then echo "Error: .env not found" >&2; exit 1; fi`
2. Replace bulk `export POSTGRES_PASSWORD REDIS_PASSWORD` (lines 57-78) with selective export — only export non-secret vars (`COMPOSE_DEV`, `COMPOSE_MON`, etc.). For secrets, source `.env` directly and pass to docker compose via `--env-file .env` instead of `export`.
3. Alternatively, prefix secret reads with `local` so they don't leak: `local POSTGRES_PASSWORD="${POSTGRES_PASSWORD:-}"`

### S1.8 Add `depends_on` with health checks in prod compose
**File:** `compose/prod.yml`
**Fix:** Add `depends_on` blocks with `condition: service_healthy` for `postgres`, `redis`, and `rustfs` to the `backend` service. Also add `depends_on` for the Python sidecar. The backend service already has these for nginx — but the backend itself should wait for DB/Redis/storage to be healthy before starting.

### S1.9 Configure alertmanager receivers with template
**File:** `compose/monitoring/alertmanager.yml`
**Fix:** Add commented-out Slack receiver with clearer instructions. Add a `dev-receiver` that logs to stdout with `webhook_configs` pointing to a local health-check script. Document that teams should uncomment and configure Slack/PagerDuty for production.

### S1.10 Blacklist check: make fail-open configurable
**File:** `backend/auth/src/middleware.rs`
**Fix:**
1. Add `fail_open_on_redis_error: bool` field to `AuthConfig` (default `true` for backwards compatibility).
2. Read from `AUTH_FAIL_OPEN_ON_REDIS_ERROR` env var in `AuthConfig::from_env()`.
3. In `auth_middleware`, replace the `unwrap_or(None)` / `None` branch with:
   - If `fail_open_on_redis_error`: log warning, continue (current behavior)
   - If not: return `next.run(req).await` without `AuthUser` extension, effectively rejecting the request
4. Document the flag in `.env.example`.

### S1.11 Fix `.env` vs `.env.example` inconsistencies
**Files:** `.env`, `.env.example`
**Fix:**
1. Add missing keys to `.env.example`: `DB_MAX_CONNECTIONS`, `REDIS_KEY_PREFIX`, `REDIS_POOL_SIZE`, `SIDECAR_SHARED_SECRET` (already there).
2. Add missing OAuth sections to `.env`: `GOOGLE_CLIENT_ID`, `GOOGLE_CLIENT_SECRET`, `GITHUB_CLIENT_ID`, `GITHUB_CLIENT_SECRET`, `OAUTH_REDIRECT_URL` (commented out).
3. Add `FAIL_OPEN_ON_REDIS_ERROR` to both files.

---

## Phase 2 — Bug Fixes & Data Safety (7 items)

### B2.1 Mark `prometheus-client` Dockerfile item as FIXED
The Dockerfile.python builder stage (line 19) installs `prometheus-client>=0.21,<0.26` into the venv, which is copied to the runtime stage. This item is already fixed — update TODOS.md `[ ]` → `[x]`.

### B2.2 Fix `window.fetch` monkey-patch error handling
**File:** `frontend/src/components/Layout.astro`
**Fix:**
1. Replace `body: JSON.stringify({})` in `performRefresh()` (line 139) with empty body or no body — the refresh endpoint reads cookies, not JSON body.
2. Replace the silent `catch {}` at line 165-167 with `catch (err) { console.warn("Retry failed:", err.message); return res; }` — always log the error type, never silently swallow.
3. Add a comment to the `catch { /* documented */ }` blocks explaining the reasoning.

### B2.3 Deduplicate health-check logic
**Files:** `frontend/src/lib/health.ts`, `frontend/src/pages/index.astro`
**Fix:**
1. Remove the `isFullOutage()` and `getDiagnostics()` re-implementations from `index.astro` (lines 258-275).
2. Import both functions from `health.ts` in `index.astro` (already does this for `isFullOutage` on line 227, but calls `checkFullOutage` locally — remove the local wrapper and use the import directly).
3. The `getDiagnostics` function in `index.astro` (lines 262-271) differs from `health.ts` — unify by adding a `detail` param to `getDiagnostics` in `health.ts` that includes the fix/error rendering, then remove the inline version.

### B2.4 Standardize empty catch blocks in frontend
**Files:** `frontend/src/components/AuthForm.astro`, `frontend/src/components/Layout.astro`
**Fix:** Adopt a convention:
- `catch { return null; }` for expected failure paths (network fetches where null is the fallback)
- `catch (err) { console.warn("context:", err instanceof Error ? err.message : String(err)); }` for unexpected errors
- `catch { /* intentional: reason */ }` only when truly ignorable (localStorage in private browsing)
- Never use bare `catch {}`

Apply pattern:
1. `AuthForm.astro:69` — `catch { return []; }` is fine (network failure fallback). Add comment: `/* network failure — show no providers */`
2. `Layout.astro:121` — `catch { /* server session cleanup is best-effort */ }` — fine, already documented
3. `Layout.astro:149` — already uses `console.warn`
4. `Layout.astro:165-167` — see B2.2 above
5. `Layout.astro:176` — `catch { /* localStorage may be blocked */ }` — fine
6. `Layout.astro:183` — same pattern — fine
7. `dashboard.astro:119` — `catch { ... guard display }` — fine

### B2.5 Fix Dockerfile.python duplicate dependency installation
**File:** `compose/Dockerfile.python`
**Fix:** The builder stage (line 19) installs `fastapi`, `uvicorn`, `prometheus-client`. The runtime stage copies the venv. There is NO re-installation — the runtime stage just copies `py-api/` source. This is actually correct. Mark as FIXED in TODOS.md. The perceived "waste" is intentional: the builder stage creates a complete venv, and the runtime stage uses it.

Actually, re-examining: The Dockerfile looks correct now. Builder creates venv, runtime copies it. The `prometheus-client` is in the builder's `uv pip install`. No duplicate installation. Mark as FIXED.

### B2.6 Fix Dockerfile health check hardcoded socket path
**File:** `compose/Dockerfile.python`
**Fix:** The health check already uses `os.environ.get('PYTHON_SIDECAR_SOCKET','/tmp/sidecar/py-api.sock')`. The default matches the `ENV` set on line 42. This is actually fine — the default is just a fallback. Mark as FIXED in TODOS.md since it's configurable via environment. Alternatively, simplify by removing the default since `ENV` already sets it:
```
HEALTHCHECK --interval=30s --timeout=5s --retries=3 \
    CMD python -c "import socket,os; s=socket.socket(socket.AF_UNIX); s.connect(os.environ['PYTHON_SIDECAR_SOCKET'])" || exit 1
```
This removes the hardcoded fallback, since the `ENV` on line 42 always sets the value.

### B2.7 Module-level `SHARED_SECRET` in Python
**File:** `py-api/app/main.py`
**Fix:** The pattern is already improved — `SHARED_SECRET` is set via `_get_shared_secret()` at module level, and `setup_logging()` is on the startup event. The remaining concern is testability. The `conftest.py` already handles this with `monkeypatch`. Mark as LOW priority and partially resolved. To fully fix, wrap in a class or use FastAPI dependency injection:
```python
class Settings:
    def __init__(self):
        self.shared_secret: str = os.environ.get("SIDECAR_SHARED_SECRET", "")

settings = Settings()
```
Then reference `settings.shared_secret` instead of `SHARED_SECRET`. This makes tests cleaner and removes the module-level global.

---

## Phase 3 — Code Quality (8 items)

### C3.1 Extract hardcoded `localhost:8001` into config
**Files:** `frontend/src/lib/health.ts:169`, `frontend/src/pages/api/auth/[...route].ts:6`, `astro.config.mjs:14`
**Fix:**
1. In `health.ts`, the default parameter `apiBase = "http://localhost:8001"` is already the function default. The actual call site uses `import.meta.env.VITE_RUST_BACKEND_URL` via a proxy. No change needed for production — it's a dev fallback. Add a comment: `// Dev fallback; production uses env var`.
2. In `[...route].ts`, same pattern — `import.meta.env.VITE_RUST_BACKEND_URL || "http://localhost:8001"`. This is fine. Add comment.
3. In `astro.config.mjs`, this is dev proxy config. Add a comment: `// Dev proxy only; production uses nginx routing`.

Alternatively, extract to a shared constant: `frontend/src/lib/config.ts`:
```typescript
export const API_BASE = import.meta.env.VITE_RUST_BACKEND_URL || "http://localhost:8001";
```
Then import from both files.

### C3.2 Deduplicate `SERVICE_IDS` list
**Files:** `frontend/src/lib/health.ts:1`, `frontend/src/pages/index.astro`
**Fix:** Remove the local `SERVICE_IDS` usage from `index.astro`. The import on line 227 already imports `SERVICE_IDS` from `health.ts`. Verify the inline usage on lines 308-309 uses the imported constant. Remove any duplicate definition.

### C3.3 Add typed interfaces for health check responses
**File:** `frontend/src/lib/health.ts`, `frontend/src/pages/index.astro`
**Fix:** The `HealthEntry` and `HealthResponse` types already exist in `health.ts` (lines 13-27). The issue is that `index.astro` still uses `Record<string, unknown>` for the response data. Fix by importing `HealthResponse` and typing the `fetchHealth()` result:
```typescript
const data: HealthResponse = await res.json();
```
Then update all property accesses to use the typed interface instead of `as Record<string, unknown>` casts.

### C3.4 Guard `jsonLog` for production
**File:** `frontend/src/lib/health.ts:29-33`
**Fix:** The `jsonLog` function already checks `import.meta.env.DEV`. But `import.meta.env.DEV` is `true` in dev and `false` in production builds. However, this is SSR code that runs on both server and client. The check should be:
```typescript
function jsonLog(obj: Record<string, unknown>): void {
  if (typeof window === "undefined" && import.meta.env.DEV) {
    console.log(JSON.stringify(obj));
  }
}
```
This ensures it only logs on the server in dev mode, never on the client or in production SSR.

### C3.5 Add Python docstrings and return type annotations
**File:** `py-api/app/main.py`
**Fix:**
1. Add docstring to `setup_logging()` (line 66 — already present, verify content).
2. Add docstring to `JsonFormatter.format()` — already has one. Verify completeness.
3. Add return type annotations to middleware functions:
   ```python
   async def trace_id_middleware(request: Request, call_next) -> Response:
   ```
   already returns Response. The issue is the type annotation on `call_next`:
   ```python
   from starlette.types import ASGIApp, Receive, Scope, Send
   async def trace_id_middleware(request: Request, call_next: Callable[[Request], Awaitable[Response]]) -> Response:
   ```
   Same for `hmac_auth_middleware`.

### C3.6 Fix `backoff_increment` doc comment
**File:** `backend/cache/src/rate_limit.rs:183-186`
**Fix:** Remove `CacheError::BackoffBlocked` from the `# Errors` doc on `backoff_increment` since it never returns that variant. Only `backoff_check` returns `BackoffBlocked`.

### C3.7 Rename `AuthMode::Both` or document security implications
**File:** `backend/auth/src/lib.rs:43-49`
**Fix:** Add a comprehensive doc comment explaining the security trade-offs:
```rust
/// Both cookie and bearer auth. Bearer takes precedence when both are present.
///
/// # Security considerations
///
/// When both a cookie and a Bearer token are sent, the Bearer token is used.
/// This means CSRF protection is bypassed for requests that include a Bearer
/// header. This is safe because:
/// 1. Bearer tokens can't be sent cross-origin without JS access
/// 2. The `SameSite=Lax` cookie attribute provides additional CSRF protection
/// 3. Cookie-auth requests still require CSRF validation for state-changing methods
Both,
```

### C3.8 Convert `unwrap()` to proper error handling in `main.rs`
**File:** `backend/api/src/main.rs`
**Fix:** Replace `.expect()` calls with `unwrap_or_else` + `process::exit(1)`:
```rust
let addr: SocketAddr = "0.0.0.0:8001".parse().unwrap_or_else(|e| {
    tracing::error!(error = %e, "invalid listen address");
    std::process::exit(1);
});
let listener = tokio::net::TcpListener::bind(addr).await.unwrap_or_else(|e| {
    tracing::error!(error = %e, "failed to bind listen address");
    std::process::exit(1);
});
```
For the SIGTERM handler and server run, same pattern. This prevents panics and produces log output before exit.

### C3.9 Consolidate frontend test runners
**Files:** `frontend/package.json`, `.github/workflows/ci.yml`
**Fix:** Choose one runner. Vitest is the primary test runner (in `vitest.config.ts`). Remove references to `bun test` for unit tests. In CI, use `npx vitest run` exclusively. In `AGENTS.md`, update `test: cd frontend && vitest run` and remove `test: cd frontend && bun test`. In `package.json`, remove any `test:ci` or `test:bun` scripts that reference `bun test`.

---

## Phase 4 — Infrastructure Fixes (5 items)

### I4.1 Increase default `DB_MAX_CONNECTIONS`
**Files:** `backend/api/src/lib.rs:47`, `.env:8`
**Fix:**
1. Change the default from `5` to `20` in `lib.rs:47` (already 20 per the agent — verify).
2. Update `.env` comment to explain the setting.
3. The key already exists in `.env` as `DB_MAX_CONNECTIONS=20`. Verify the code reads it: `std::env::var("DB_MAX_CONNECTIONS").ok().and_then(|v| v.parse().ok()).unwrap_or(20)`. Already using 20.

### I4.2 Deduplicate health check rendering in Rust
**File:** `backend/api/src/lib.rs:355-449`
**Fix:** Extract the common JSON construction from `health_python()` (lines ~419-423) to call `health_python_value()` instead. The traced and untraced branches should share the same rendering logic. Refactor:
```rust
// Before: inline JSON construction in traced branch
// After: call health_python_value() for both branches
```

### I4.3 Add backup/restore scripts
**File:** `scripts/backup.sh` (new), `scripts/restore.sh` (new)
**Fix:** Create two scripts:
1. `scripts/backup.sh` — Uses `pg_dump` to back up PostgreSQL, `redis-cli BGSAVE` + copy for Redis, and `aws s3` / `mc` for RustFS bucket backup. Writes to `.backup/` with timestamps.
2. `scripts/restore.sh` — Restores from a specified backup file for each service.

### I4.4 Add `conftest.py` shared fixtures (already exists)
**File:** `py-api/tests/conftest.py`
**Fix:** The `conftest.py` already exists with an `autouse` fixture. The TODO item says "no shared test fixtures" but there IS a conftest.py. Mark as FIXED in TODOS.md. Optionally, add a docstring to conftest.py explaining the shared fixture pattern.

### I4.5 Standardize shebangs
**File:** `scripts/status.sh`
**Fix:** All other scripts use `#!/bin/bash` (via `set -euo pipefail` on line 2). `status.sh` uses `#!/usr/bin/env bash`. Either is valid, but be consistent. Change `status.sh` to `#!/bin/bash` to match the rest, or change all others to `#!/usr/bin/env bash`. The `env` approach is more portable — change all scripts to `#!/usr/bin/env bash`.

---

## Phase 5 — Performance (3 items)

### P5.1 Reduce `format!()` allocations in hot paths
**File:** `backend/auth/src/routes.rs:184,193,338,347`
**Fix:** The cookie header construction uses `format!()` per request. Since the format strings are simple (3 string interpolations), this is a micro-optimization. Use `std::fmt::Write` with a `String` that's cleared between uses, or use a `HeaderValue::from_str()` directly:
```rust
// Before:
format!("access_token={}; HttpOnly; Path=/; Max-Age={}; SameSite=Lax", access_token, config.jwt_expiry)

// After: use HeaderValue::from_str which avoids double-allocation
let cookie = format!("access_token={access_token}; HttpOnly; Path=/; Max-Age={max_age}; SameSite=Lax");
headers.insert(header::SET_COOKIE, cookie.parse().unwrap());
```
The `unwrap()` on the parse is safe because the format is always valid. Lower priority — the performance gain is negligible for <1000 rps.

### P5.2 Extract inline CSS from Layout.astro
**File:** `frontend/src/components/Layout.astro`
**Fix:** Move the ~273 lines of inline CSS to `frontend/src/styles/layout.css` and import it via `<link rel="stylesheet" href="/styles/layout.css" />` (already done — Layout.astro line 16 does this). Wait — Layout.astro already has `<link rel="stylesheet" href="/styles/layout.css" />` on line 16 AND an inline `<style>` block. The inline `<style>` should be removed if the same styles are in the linked CSS. Check if the styles are duplicated. If they are, remove the inline block. If the inline block has unique styles, extract them to the CSS file.

### P5.3 Fix health retry UI re-render flash
**File:** `frontend/src/pages/index.astro`
**Fix:** In the `fetchHealth()` function, when a full outage triggers a retry, the current code sets all status dots to "loading" which causes a visual flash. Instead, preserve the current error status and only update once fresh data arrives:
```typescript
// Instead of resetting all dots to "loading" before fetch:
// for (const svc of SERVICE_IDS) {
//   setStatus(svc, "loading", "rechecking…");
// }
// Just keep current status and let the fetch update when results arrive.
```
Remove or comment out any "reset to loading" logic before `fetchHealth()` in the retry path. The `resetRetry()` function should reset the timer but NOT reset UI state.

---

## Phase 6 — Documentation (9 items)

### D6.1 Fix `performance-budget.md` to reference `ab`
**File:** `docs/performance-budget.md`
**Fix:** Update the document to explicitly state that `bench.sh` uses Apache Bench (`ab`), not `bombardier`. Remove or update any references to `bombardier`. The change is purely documentation — the tool is already correct.

### D6.2 Document socket path differences between dev and prod
**File:** `docs/ARCHITECTURE.md`
**Fix:** Add a section explaining:
- Dev: `/tmp/fullstackhex-python.sock` (set in `.env` as `PYTHON_SIDECAR_SOCKET`)
- Prod: `/tmp/sidecar/py-api.sock` (set in `compose/prod.yml`)
- Both are configurable via the `PYTHON_SIDECAR_SOCKET` env var
- Include a note about Docker volume mounts for the socket directory

### D6.3 Update `INFRASTRUCTURE.md` embedded compose section
**File:** `docs/INFRASTRUCTURE.md`
**Fix:** Update the embedded dev.yml section to include:
- `ADMINER_PORT` environment variable
- `REDIS_COMMANDER_PORT` environment variable
- Redis exporter service
- PostgreSQL exporter service
- Updated port mappings showing `127.0.0.1` bindings

### D6.4 Add disaster recovery documentation
**File:** `docs/DISASTER_RECOVERY.md` (new)
**Fix:** Create a new document covering:
- PostgreSQL backup/restore with `pg_dump` / `pg_restore`
- Redis persistence (AOF/RDB) and disaster recovery
- RustFS data backup strategies
- Container failure recovery (docker compose restart)
- Horizontal scaling considerations
- Reference the backup scripts from I4.3

### D6.5 Add secrets rotation guide
**File:** `docs/SECRETS_ROTATION.md` (new)
**Fix:** Create a guide covering:
- JWT_SECRET rotation (requires all users to re-authenticate)
- DATABASE_URL password rotation (requires PostgreSQL ALTER USER + .env update + restart)
- REDIS_PASSWORD rotation (update .env + restart Redis)
- RUSTFS_ACCESS_KEY/SECRET_KEY rotation (update .env + restart RustFS)
- SIDECAR_SHARED_SECRET rotation (update .env + restart both backend and py-api)
- TLS certificate renewal (certbot auto-renewal + nginx reload)

### D6.6 Add TLS renewal documentation
**File:** `docs/TLS.md` (new) or section in `docs/INFRASTRUCTURE.md`
**Fix:** Document:
- Certbot container auto-renews every 12 hours (see entrypoint in prod.yml)
- Monitoring: check certbot logs, add alert rule for cert expiry
- Manual renewal: `docker compose -f compose/prod.yml exec certbot certbot renew`
- Certificate path: `./nginx/certs/`
- Nginx container needs `SIGHUP` or restart after cert renewal

### D6.7 Rename `DESIGN.md` or add redirect
**File:** `docs/DESIGN.md`
**Fix:** Rename to `docs/DOC_STYLE_GUIDE.md` and add a `docs/DESIGN.md` that contains a redirect:
```markdown
# System Design

See [ARCHITECTURE.md](./ARCHITECTURE.md) for system architecture and design decisions.

> **Note:** The document previously at this location was a documentation style guide.
> It has been moved to [DOC_STYLE_GUIDE.md](./DOC_STYLE_GUIDE.md).
```

### D6.8 Add Grafana/Prometheus version specifics to MONITORING.md
**File:** `docs/MONITORING.md`
**Fix:** Update the line "Prometheus 3.x + Grafana" to specify exact versions matching the compose files:
- Prometheus: `v3.3.1` (see `compose/monitor.yml`)
- Grafana: `11.2.0` (see `compose/monitor.yml`)

### D6.9 Add `# Errors` doc sections where missing
**File:** `backend/cache/src/rate_limit.rs`
**Fix:** The doc comments already have `# Errors` sections for `rate_limit_check`, `rate_limit_count`, `backoff_check`, and `backoff_increment`. But `backoff_increment`'s `# Errors` section incorrectly says it can return `BackoffBlocked` — it cannot. Fix this. Also check other public functions in auth routes for missing `# Errors` sections.

---

## Phase 7 — Naming & Env Consistency (2 items)

### N7.1 Rename `PYTHON_SIDECAR_*` environment variables to `PY_API_*`
**Files:** `.env`, `.env.example`, `compose/dev.yml`, `compose/prod.yml`, `py-api/app/main.py`, `backend/api/src/lib.rs`, `backend/py-sidecar/src/lib.rs`, `compose/Dockerfile.python`
**Fix:** This is a large breaking change. Instead of a full rename, add compatibility shims:
1. In `.env`, add `PY_API_SOCKET=${PYTHON_SIDECAR_SOCKET}` and `PY_API_SHARED_SECRET=${SIDECAR_SHARED_SECRET}` as aliases.
2. In code, read `PY_API_SOCKET` first, fall back to `PYTHON_SIDECAR_SOCKET`.
3. Add deprecation notice in `.env` comments.
4. Update `docs/ARCHITECTURE.md` to reference both names.

Alternatively, skip this for now — it's a LOW priority naming inconsistency that would break existing deployments. Document the dual naming in ARCHITECTURE.md and defer the rename.

### N7.2 Resolve `.env` vs `.env.example` inconsistencies
**Files:** `.env`, `.env.example`
**Fix:** Comprehensive sync:
1. Add to `.env.example` all keys present in `.env`: `DB_MAX_CONNECTIONS`, `REDIS_KEY_PREFIX`, `REDIS_POOL_SIZE`, `RUSTFS_*` vars, `SIDECAR_SHARED_SECRET`.
2. Add to `.env` the commented OAuth sections: `GOOGLE_CLIENT_ID`, `GOOGLE_CLIENT_SECRET`, `GITHUB_CLIENT_ID`, `GITHUB_CLIENT_SECRET`, `OAUTH_REDIRECT_URL`.
3. Ensure every key in `.env` has a corresponding entry (even commented) in `.env.example`, and vice versa.

---

## Implementation Order

| Step | Items | Risk | Effort |
|------|-------|------|--------|
| 1 | S1.1-S1.3 (pin tags, Redis pwd, bind localhost) | Low | 30min |
| 2 | S1.6, S1.7 (eval removal, config.sh secrets) | Medium | 45min |
| 3 | B2.2, B2.3 (fetch monkey-patch, health dedup) | Low | 30min |
| 4 | C3.1-C3.3 (hardcoded localhost, SERVICE_IDS, typed interfaces) | Low | 45min |
| 5 | C3.8 (unwrap() → error handling) | Medium | 20min |
| 6 | S1.8, S1.10 (depends_on, configurable blacklist) | Medium | 30min |
| 7 | S1.9, S1.11 (alertmanager, .env sync) | Low | 30min |
| 8 | S1.5, I4.4 (mark already-fixed items) | None | 5min |
| 9 | I4.2 (Rust health check dedup) | Low | 20min |
| 10 | C3.4-C3.7 (jsonLog, docstrings, AuthMode docs, backoff_increment doc) | Low | 30min |
| 11 | B2.4, B2.7 (catch blocks, SHARED_SECRET DI) | Low | 20min |
| 12 | P5.1-P5.3 (format! allocations, CSS extraction, UI flash) | Low | 45min |
| 13 | I4.3, I4.5 (backup scripts, shebangs) | Low | 60min |
| 14 | C3.9 (consolidate test runners) | Low | 15min |
| 15 | D6.1-D6.9 (all documentation) | None | 90min |
| 16 | N7.1-N7.2 (naming, .env sync) | Low | 30min |
| 17 | S1.4, B2.1, B2.5, B2.6 (mark already-fixed/closed items) | None | 5min |

**Total estimated time: ~8 hours**

---

## Already-Fixed Items to Mark Closed

These items are actually fixed but marked `[ ]` in TODOS.md:
1. **`prometheus-client` missing from Dockerfile** — Line 19 includes `prometheus-client>=0.21,<0.26`. Mark `[x]`.
2. **Dockerfile.python duplicate dependency installation** — Builder creates venv, runtime copies it. No duplication. Mark `[x]`.
3. **Node-exporter mounts without read-only** — All mounts use `:ro`. Mark `[x]`.
4. **Certbot has no resource limits** — It does now (lines 360-364). Mark `[x]`.
5. **Dockerfile health check hardcoded socket path** — It uses `os.environ.get()` with a configurable `ENV`. Mark `[x]`.
6. **Missing `conftest.py` in py-api/tests/** — `conftest.py` exists with autouse fixture. Mark `[x]`.
7. **`backoff_increment` doc says BackoffBlocked** — This is a doc bug to fix, not a missing item. Update the doc.

## GSTACK REVIEW REPORT

| Review | Trigger | Why | Runs | Status | Findings |
|--------|---------|-----|------|--------|----------|
| Design Review | `/plan-design-review` | UI/UX gaps | 1 | ISSUES (resolved) | score: 2/10 → 7/10, 9 decisions, 0 unresolved |

**DECISIONS MADE (all resolved in code):**
1. Nav label: "Dashboard" → "Health" (Layout.astro)
2. Merge `/dashboard` → `/profile`, delete dashboard.astro (redirects updated)
3. Loading skeleton on profile page (profile.astro)
4. Homepage subtitle already present — no change needed
5. Form label already present — no change needed
6. `--text-muted` lightened from #64748b → #94a3b8 (WCAG AA)
7. Status dots aria-live region + sr-only spans (index.astro)
8. Skip-to-content link (Layout.astro)
9. Hardcoded colors → CSS vars across all notes pages

**VERDICT:** CLEARED — all design issues resolved in code during review pass.