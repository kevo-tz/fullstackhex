# TODO

## P0 — Blocks Deployment (fix before any `docker compose up`)

- [x] **P0: Python sidecar crashes at startup** — `compose/Dockerfile.python:13-15` never installs py-api package; `py-api/app/main.py:66` calls `version("py-api")` → `PackageNotFoundError`. Fix: add `RUN uv pip install -e .` after dep install. Wrap `version()` in try/except with `"0.0.0"` fallback.

- [x] **P0: nginx SPA fallback broken** — `compose/nginx/nginx.conf:128` proxies to `http://frontend/index.html` but Astro SSR produces `dist/server/entry.mjs`, not `index.html`. Fix: change to `proxy_pass http://frontend` (proxy to SSR server).

- [x] **P0: Frontend Docker crashes on first request** — `compose/Dockerfile.frontend:27-28` copies only `dist/` and `package.json`. `dist/server/entry.mjs` has bare-imports (`piccolore`, `@astrojs/*`, `devalue`). Runtime needs `node_modules`. Fix: run `npm install --production` in runtime stage or copy `node_modules` from builder.

- [x] **P0: `SIDECAR_SHARED_SECRET` missing in prod.yml** — `compose/prod.yml:111-113` py-api env block has `PYTHON_LOG_LEVEL` and `PYTHON_SIDECAR_SOCKET` but not `SIDECAR_SHARED_SECRET`. HMAC middleware rejects all non-public requests. Fix: add `SIDECAR_SHARED_SECRET: ${SIDECAR_SHARED_SECRET}` to environment.

- [x] **P0: `monitoring/prometheus.yml` is an empty directory** — The real config is at `compose/monitoring/prometheus.yml`. Fix: delete the empty `monitoring/prometheus.yml` directory.

## P1 — Breaks Core Features

- [x] **P1: WebSocket cookie auth always returns None** — `api/src/live.rs:269` calls `cache_get::<String>("session", ...)` which tries `serde_json::from_str::<String>` on a JSON object. Session stores `{user_id, email, name, provider, created_at}` as JSON, but code tries to read it as a `String`. Deserialization always fails → `None` → 401. Fix: use `session_get` to retrieve `Session` struct, then reconstruct `AuthUser`.

- [x] **P1: Blacklist check conflates cache miss with Redis error** — `auth/src/middleware.rs:125-128`: `.unwrap_or(None)` makes `Ok(None)` (not blacklisted) and `Err(...)` (Redis down) both produce `None`. Default config: spurious `"blacklist check failed"` warning on every request. Fail-closed config (`AUTH_FAIL_OPEN_ON_REDIS_ERROR=false`): all valid requests get 401. Fix: match on `Ok(None)` → allow, `Err(...)` → use fail_open/fail_closed gate.

- [x] **P1: CSRF token extraction fails with HttpOnly cookies** — `frontend/src/pages/notes/create.astro:63`, `[id].astro:106` read `csrf_token` from `document.cookie`. If set HttpOnly (best practice), JS can't read it → `X-CSRF-Token` empty. Fix: verify backend cookie flags; consider returning CSRF in response body instead.

- [x] **P1: Logout can't clear HttpOnly auth cookies from JS** — `frontend/src/components/Layout.astro:126-127` tries `document.cookie = "access_token=; max-age=0"` — silently fails for HttpOnly cookies. Fix: remove these lines. Backend already blacklists JWT and deletes refresh token.

- [x] **P1: Redis `--save` malformed in prod.yml** — `compose/prod.yml:218`: `--save ${REDIS_SAVE:-900 1 300 10 60 10000}` passes all values as single arg. Redis only reads first pair `900 1`. Last 3 save points silently dropped. Fix: use multiple `--save` flags or a proper redis.conf.

- [x] **P1: TLS private keys not gitignored** — `.gitignore:27-30` patterns `nginx/certs/*.pem` don't match `compose/nginx/certs/`. `git check-ignore` confirms not ignored. Fix: add `compose/nginx/certs/*.pem` etc. to `.gitignore`.

- [x] **P1: Toast component entirely non-functional** — `frontend/src/components/Toast.astro:9` renders `<div id="toast-container">` but `Toast.astro:40` defines `<toast-container>` custom element. `frontend/src/pages/notes/[id].astro:92-94` does `querySelector("toast-container")` → null → `show()` never fires. Fix: change HTML template to use `<toast-container>` tag, OR change selector to `#toast-container`.

- [x] **P1: OAuth UPSERT silently overwrites provider** — `auth/src/routes.rs:684-688`: `ON CONFLICT (email) DO UPDATE SET provider = EXCLUDED.provider`. A Google-registered user's provider gets overwritten to GitHub if a different GitHub account has the same email. Fix: remove `provider = EXCLUDED.provider` from UPDATE SET, or consider email-as-identity model.

## P2 — Test Quality (False Confidence)

- [x] **P2: 4 vitest files test duplicates, not real code** — `auth-gating.vitest.ts`, `dashboard.vitest.ts`, `auth-form.vitest.ts`, `theme.vitest.ts` all re-implement `renderDashboard()`, `setStatus()`, `showError()`, `initTheme()` instead of importing from components. Tests pass even if production code breaks. Fix: refactor component code to export functions, then import in tests. Or remove if not salvageable.

- [x] **P2: Playwright notes tests swallow errors** — `frontend/tests/e2e/playwright/notes.spec.ts:25,43,58` use `.catch(() => {})`. Failures silently hidden. Fix: replace with `console.error` or proper assertion.

## P3 — Code Quality & Observability

- [x] **P3: `PYTHON_LOG_LEVEL` env var defined in 3 places, consumed nowhere** — `py-api/app/main.py:98` hardcodes `logging.INFO`. Fix: read env var with `getattr(logging, os.environ.get("PYTHON_LOG_LEVEL", "INFO").upper())`.

- [x] **P3: `register_metrics()` swallows all ValueError silently** — `py-api/app/main.py:59-60`: `except ValueError: pass`. No logging. Fix: log the error, catch only duplicate registration errors.

- [x] **P3: `normalize_route` missing 10+ route patterns** — `api/src/metrics.rs:50-72`. Notes, storage multipart, OAuth routes all map to `"unknown"`. Fix: add match arms for `/notes`, `/notes/*`, `/storage/multipart/*`, `/auth/providers`, `/auth/oauth/*`.

- [x] **P3: Logout doesn't clear all cookies** — `auth/src/routes.rs:443-450` only clears `session` cookie. `access_token`, `refresh_token`, `csrf_token` cookies remain stale. Fix: add `remove_cookie` calls.

- [x] **P3: Delete note returns 200 instead of 204** — `api/src/notes.rs:258`. REST convention for DELETE is 204 No Content. Fix: `(StatusCode::NO_CONTENT, "")`.

- [x] **P3: Registration returns 422 instead of 409 for duplicate email** — `auth/src/routes.rs:150`. Fix: return `ApiError::Conflict("Email already registered")` — requires adding `Conflict` variant to `ApiError`.

- [x] **P3: `AUTH_FAIL_OPEN_ON_REDIS_ERROR` only parses "true"/"false" case-sensitively** — `auth/src/lib.rs:157-160`. `"TRUE"`, `"1"`, `"0"` silently ignored, default to true. Fix: case-insensitive parsing.

- [x] **P3: Py-api version drift** — `py-api/pyproject.toml:7` says `0.13.0`, root `VERSION` says `0.13.6`. Fix: bump pyproject.toml to `0.13.6`.

- [x] **P3: Prometheus version pin drift** — `pyproject.toml:13` pins `prometheus-client<0.27`, `Dockerfile.python:15` pins `<0.26`. Fix: align to same upper bound.

- [x] **P3: `PY_API_SOCKET` alias documented but never consumed** — `.env.example:42` says `PYTHON_SIDECAR_SOCKET` is deprecated in favor of `PY_API_SOCKET`, but no code reads it. Fix: either add code to read it, or remove alias from docs.

- [x] **P3: `down.sh` kills system-wide by process name** — `scripts/down.sh:22-26`: `pkill -x api` kills ALL processes named `api` on the machine. Fix: use PID files or Docker compose to stop services.

- [x] **P3: Alertmanager webhook targets container's own localhost** — `compose/monitoring/alertmanager.yml:24`: `http://localhost:5001/alerts` resolves to alertmanager container itself, not the receiver. Fix: change to reachable endpoint or add receiver service.

- [x] **P3: Prometheus rule file `alerts.yml` not mounted** — `compose/monitoring/prometheus.yml:16` references `alerts.yml`, but `monitor.yml` doesn't mount it. Fix: add volume mount.

- [x] **P3: Docs outdated** — CI.md says 7 jobs (actual: 6), INFRASTRUCTURE.md lists stale image versions, `CLAUDE.md:75` references `scripts/pre-push-check.sh` which doesn't exist. Fix: sweep docs.

- [x] **P3: Registration timing side-channel** — Rate limit (instant 429) fires before validation (~50ms 422). Attacker can distinguish rate-limited from invalid-input. Document as inherent tradeoff.
