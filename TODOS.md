# TODOS

## Completed

- Wire PythonSidecar IPC + real DB health checks (v0.3.1.0)
- Parallelize frontend health fetches (v0.5.0.0)
- Add `make watch` target (v0.5.0.0)
- Document log locations per service (v0.5.0.0)
- Fix outdated Python sidecar docs in SERVICES.md (v0.5.0.0)
- Add `make logs-python` target (v0.5.0.0)

## Active (from /qa v0.8 review)

### A1. Fix migration 001 format — sqlx runs entire file as up-migration
**What:** `backend/crates/db/migrations/001_create_users.sql` contains both `CREATE TABLE` and `DROP TABLE` in the same file with golang-migrate style comments (`-- +migrate Up/Down`). sqlx ignores these comments and runs the entire file as an up-migration, causing the table to be created then immediately dropped.
**Fix:** Split into `001_create_users.up.sql` / `001_create_users.down.sql` (sqlx standard). Remove the `-- +migrate` comments. Verify by running `make migrate` on a fresh database.
**Files:** `backend/crates/db/migrations/001_create_users.sql`
**Severity:** Critical — breaks auth on fresh installs.
**Found by:** /qa on 2026-05-04.

### A2. Replace `check-env` placeholder-only validation with required-variable schema
**What:** `Makefile:127` `check-env` only checks for `CHANGE_ME` strings. It does NOT verify that required variables like `JWT_SECRET`, `DATABASE_URL`, or `REDIS_URL` actually exist. Auth was silently disabled because `JWT_SECRET` was present in `.env.example` but missing from the local `.env` — `check-env` passed, backend started, but `/auth/*` returned 404.
**Fix:** Add a `scripts/validate-env.py` (or shell) script that reads `.env.example` for required keys and ensures each exists in `.env` with a non-empty value. Run it in `make dev` and `make up` before starting services.
**Files:** `Makefile`, `.env.example`
**Severity:** High — silent feature disable is worse than a crash.
**Found by:** /qa on 2026-05-04.

### A3. Add `.env` sync command — `make sync-env`
**What:** `.env` is gitignored. When `.env.example` changes (e.g., v0.8 adds auth vars), developers must manually diff and merge. There is no automated way to detect drift.
**Fix:** Add `make sync-env` that compares `.env` against `.env.example` and prints missing keys with their example values. Optionally support `make sync-env --apply` to append missing keys.
**Files:** `Makefile`, new `scripts/sync-env.sh`
**Severity:** Medium — friction on every env change.
**Depends on:** A2.

### A4. Quote `REDIS_SAVE` in `.env.example` and validate shell-safe values
**What:** `.env:18` `REDIS_SAVE=900 1 300 10 60 10000` contains unquoted spaces. When `.env` is sourced via `. ../.env`, the shell parses the spaces as separate commands, producing `../.env: line 18: 1: command not found`. This noise obscures real startup errors.
**Fix:** Quote the value in `.env.example`: `REDIS_SAVE="900 1 300 10 60 10000"`. Add a `make check-env` step that sources `.env` with `set -e` to fail fast on syntax errors.
**Files:** `.env.example`, `Makefile`
**Severity:** Medium — startup noise erodes trust in logs.
**Found by:** /qa on 2026-05-04.

### A5. Make `make dev` background processes survive terminal detachment
**What:** The `make dev` target runs `cargo run -p api &` inside a shell script. When the shell receives signals or the terminal session ends, the Rust backend process gets `SIGTERM` and shuts down (`"received shutdown signal, draining connections"`). Developers lose the backend unexpectedly.
**Fix:** Use `nohup` or `systemd-run --user` (or `s6`, `supervisord`) for each service process. Write PID files to a well-known location (`/tmp/fullstackhex-dev/`) and add `make status` to show which services are alive.
**Files:** `Makefile`, `scripts/dev-run.sh`
**Severity:** Medium — process management is the first thing a new dev hits.
**Found by:** /qa on 2026-05-04.

### A6. Add `make status` — show which services are running and on which ports
**What:** After `make dev`, there is no way to verify which services are actually alive without manually `curl`ing health endpoints or `ps`-grepping.
**Fix:** Add `make status` that prints a table: Service | PID | Port | Health | Uptime. Read from PID files started by `make dev`.
**Example output:**
```
Service          PID     Port    Health    Uptime
Rust API         12345   8001    ok        2m
Frontend         12346   4321    ok        2m
Python Sidecar   12347   (sock)  ok        2m
PostgreSQL       12348   5432    ok        2m
Redis            12349   6379    ok        2m
```
**Files:** `Makefile`, `scripts/status.sh`
**Severity:** Low — quality of life.
**Depends on:** A5.

### A7. Add auth status to the health dashboard
**What:** The frontend dashboard at `/` shows 5 service cards but gives zero indication of whether auth is enabled. When `JWT_SECRET` is missing, auth routes return 404 — the user sees "all green" but cannot register or log in.
**Fix:** Add an `Auth` card to the dashboard (or an inline banner) that shows "enabled" / "disabled" based on `/health` or a dedicated `/auth/status` endpoint. When disabled, show a one-line fix: `JWT_SECRET not set — auth disabled. Add to .env and restart.`
**Files:** `frontend/src/pages/index.astro`, `frontend/src/lib/health.ts`
**Severity:** Medium — silent feature disable is confusing.
**Found by:** /qa on 2026-05-04.

### A8. Add `make test-e2e` — full stack smoke test
**What:** The existing test suites (Rust unit, frontend unit, Python unit) run in isolation. None verify that the backend + frontend + database actually work together. `/qa` found the auth 500 bug only by manually curling endpoints.
**Fix:** Add a Playwright or Bun-based e2e test that:
1. Starts `make dev` (or uses existing running services)
2. Opens `http://localhost:4321`
3. Registers a test user via `/auth/register`
4. Logs in via `/auth/login`
5. Hits `/auth/me` with the token
6. Verifies dashboard shows all 5 services as "ok"
Run in CI on every PR.
**Files:** `e2e/`, `.github/workflows/e2e.yml`, `package.json`
**Severity:** High — prevents regressions like the UUID type mismatch from reaching main.
**Depends on:** A5 (reliable process startup).

### A9. Add contract tests for frontend health aggregation
**What:** `frontend/tests/integration-health-route.test.ts` mocks `fetch` and asserts on the response shape. When the backend adds new health endpoints (redis, storage), the tests break because they expect exactly 3 fetches. There is no automated check that frontend expectations match backend reality.
**Fix:** Generate a JSON schema from the backend `health()` return types (or an OpenAPI spec) and validate frontend mocks against it in CI. Alternatively, add a `make test-contract` that spins up the backend and runs the frontend tests against the real `/api/health` endpoint.
**Files:** `frontend/tests/integration-health-route.test.ts`, `backend/crates/api/src/lib.rs`
**Severity:** Medium — test brittleness slows refactors.
**Found by:** /qa on 2026-05-04.

### A10. Add auth login/register UI to the frontend
**What:** v0.8 ships auth backend but the frontend has no auth UI. Developers must use `curl` to test registration and login. This is fine for an API-first project but not for a "full stack" template.
**Fix:** Add a `/login` Astro page with email/password form and OAuth buttons (Google, GitHub). On success, store the JWT in `localStorage` and show a user menu. On the dashboard, gate storage actions behind auth.
**Files:** `frontend/src/pages/login.astro`, `frontend/src/components/AuthForm.astro`
**Severity:** Low — nice to have for a template.
**Depends on:** A2, A7.

### A11. Document the `make dev` signal handling quirk
**What:** `make dev` traps INT/TERM to run `make down-dev`, but the `wait` at the end means Ctrl+C kills everything including the background `cargo run`. Developers who expect `make dev` to run like `docker compose up` are surprised when the backend dies.
**Fix:** Add a 3-line note to the README and Makefile help:
```
make dev runs services in the foreground. Press Ctrl+C to stop all.
If you need services to survive terminal closure, start them individually:
  make up          # docker services only
  cd backend && cargo run -p api  # backend
  cd frontend && bun run dev      # frontend
```
**Files:** `README.md`, `Makefile`
**Severity:** Low — documentation gap.
**Found by:** /qa on 2026-05-04.

### A12. Add sqlx `query!` compile-time checking for auth routes
**What:** The UUID-to-String type mismatch in `backend/crates/auth/src/routes.rs` was caught at runtime (HTTP 500). sqlx's `query!` macro with the `offline` feature would have caught this at compile time.
**Fix:** Enable `sqlx/offline` in `backend/crates/auth/Cargo.toml`. Pre-generate `.sqlx/` query metadata with `cargo sqlx prepare`. Update CI to fail if `.sqlx/` is out of date.
**Files:** `backend/crates/auth/Cargo.toml`, `backend/Cargo.toml`, `.github/workflows/*.yml`
**Severity:** Medium — prevents data-type regressions.
**Found by:** /qa on 2026-05-04.

## Deferred

### Run ignored socket tests in CI
**What:** Start a test FastAPI instance as a CI background step so the 5 `#[ignore]` socket integration tests run automatically.
**Why:** Socket tests never run — they require `--ignored` flag. CI has Python setup but doesn't start a sidecar.
**Pros:** Catches socket regressions automatically. Closes the test coverage gap on the polyglot claim.
**Cons:** Adds ~30s to CI runs. Socket tests are timing-sensitive and may be flaky in CI.
**Context:** Tests in `python-sidecar/src/lib.rs` (4 ignored) and `integration_health_route.rs` (1 ignored). All use mock UnixListener. They pass on native Linux but fail on WSL2 due to Unix socket quirks.
**Depends on:** CI Python setup (already exists).

### Add inline Rust doc examples
**What:** `///` comments on `PythonSidecar::get()`, `PythonSidecar::health()`, and `db::health_check`.
**Why:** rust-analyzer hover shows usage examples directly in the editor. Learn by doing without docs.
**Pros:** Zero friction — developer sees example at the point of use. Updates with code changes.
**Cons:** Doc examples can rot if not compiled (use `#[doc = include_str!("...")]` or keep them simple).
**Depends on:** —

### Add concrete examples to docs
**What:** New `docs/EXAMPLES.md` or section in `docs/SERVICES.md` with copy-paste code blocks.
**Why:** No examples showing how to extend the template (add route, add sidecar endpoint, add page).
**Pros:** Reduces time to first custom feature. Shows the full extension pattern end-to-end.
**Cons:** Examples must be maintained as API evolves.
**Depends on:** —
