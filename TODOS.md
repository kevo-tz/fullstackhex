# TODOS

## Idea 1: Folder Restructure

**Completed:** v0.10.1.0 (2026-05-08)

Flatten `backend/crates/` — move all crates directly into `backend/`. Rename `python-sidecar` → `py-sidecar` (Rust crate) and `python-sidecar/` → `py-api/` (Python FastAPI project). Move root `nginx/`, `monitoring/` into `compose/` and `e2e/` into `frontend/tests/`.

### Steps

1. **Move backend crates out of `backend/crates/` and rename**
   ```bash
   mv backend/crates/api backend/crates/auth backend/crates/cache backend/crates/db backend/crates/domain backend/crates/storage backend/
   mv backend/crates/python-sidecar/ backend/py-sidecar/
   rmdir backend/crates/
   ```

2. **Move root folders into proper locations and rename python-sidecar → py-api**
    ```bash
    mv nginx/canary.conf compose/nginx/canary.conf
    mv nginx/upstream.conf.template compose/nginx/upstream.conf.template
    rmdir nginx/
    mv monitoring/ compose/monitoring/
    mv e2e/ frontend/tests/e2e/
    mv python-sidecar/ py-api/
    ```

3. **Update workspace members** — `backend/Cargo.toml` line 2
   ```toml
   members = ["api", "auth", "cache", "db", "domain", "py-sidecar", "storage"]
   ```

4. **Update crate name** — `backend/py-sidecar/Cargo.toml` line 2
   ```toml
   name = "py-sidecar"
   ```

5. **Update dependency** — `backend/api/Cargo.toml` line 22
   ```toml
   py-sidecar = { path = "../py-sidecar" }
   ```

6. **Update Rust imports** — `backend/api/src/lib.rs`
   - Line 11: `use python_sidecar::PythonSidecar` → `use py_sidecar::PythonSidecar`
   - Lines 354, 358, 362, 366, 370, 374, 404: `python_sidecar::SidecarError::` → `py_sidecar::SidecarError::`
   - SKIP lines 356, 360, 364 (these reference root Python service, not crate)

7. **Update test imports**
   - `backend/api/tests/integration_auth_routes.rs` line 7: `use python_sidecar::PythonSidecar` → `use py_sidecar::PythonSidecar`
   - `backend/api/tests/integration_storage_routes.rs` line 7: same
   - `backend/api/tests/integration_health_route.rs` lines 333, 380: `python_sidecar::PythonSidecar::new(` → `py_sidecar::PythonSidecar::new(`
   - SKIP lines 388, 427 (assert Python service name in JSON, not crate)

8. **Update crate lib.rs** — `backend/py-sidecar/src/lib.rs`
   - Lines 107, 345: doc comments `use python_sidecar::` → `use py_sidecar::`
   - Lines 363, 369, 372: tracing target `"python_sidecar"` → `"py_sidecar"`
   - Lines 514, 703: test comment `cargo test -p python-sidecar` → `cargo test -p py-sidecar`

9. **Update Dockerfile** — `compose/Dockerfile.rust`
   - Lines 19-21: remove `crates/` from all COPY paths (`backend/crates/X/Cargo.toml` → `backend/X/Cargo.toml`)
   - Lines 24-28: remove `crates/` from all mkdir + echo paths (`backend/crates/X/src` → `backend/X/src`)

10. **Add APP_NAME to Makefile** (needed by Idea 2 template substitution)
     - Insert `APP_NAME ?= fullstackhex` near top of Makefile (after `.PHONY` lines, before `COMPOSE_DEV`)

11. **Update compose references**
     - `compose/monitor.yml` lines 24, 55, 56, 76: `../monitoring/` → `./monitoring/`
     - `compose/prod.yml` line 6: comment `nginx/certs/` → `compose/nginx/certs/`

12. **Update deploy scripts + Makefile**
     - `scripts/deploy-canary.sh` line 38: `nginx/canary.conf` → `compose/nginx/canary.conf`
     - `scripts/deploy-canary-promote.sh` line 17: `nginx/upstream.conf.template` → `compose/nginx/upstream.conf.template`
     - `scripts/deploy-blue-green.sh` lines 87, 93: `nginx/upstream.conf.template` → `compose/nginx/upstream.conf.template`
     - `Makefile` line 414: remove `nginx/` from rsync, replace with `compose/nginx/`
     - `Makefile` line 379: `cd e2e` → `cd frontend/tests/e2e`

13. **Update CI/CD**
    - `Makefile` line 368: `cargo test -p python-sidecar` → `cargo test -p py-sidecar`
    - `.github/workflows/ci.yml` line 92: `cargo test -p python-sidecar` → `cargo test -p py-sidecar`
     - `.github/workflows/ci.yml` line 309: `cd e2e` → `cd frontend/tests/e2e`
     - `.github/workflows/ci.yml` line 352: `monitoring/grafana/dashboards/` → `compose/monitoring/grafana/dashboards/`

14. **Update docs** — remove `crates/` and fix moved-folder paths

    | File | Lines | What to change |
    |------|-------|---------------|
    | `docs/ARCHITECTURE.md` | 33, 124, 130, 144, 164 | remove `crates/` from all paths |
    | `docs/SERVICES.md` | 28, 34, 39, 44, 112 | remove `crates/` from all paths |
    | `docs/INITIALIZATION.md` | 83, 108-111, 204 | remove `crates/` from all paths |
    | `docs/EXAMPLES.md` | 7, 25, 229 | remove `crates/` from paths; line 255: `e2e/` → `frontend/tests/e2e/` |
    | `docs/STORAGE.md` | 3 | remove `crates/` |
    | `docs/MONITORING.md` | 193, 196, 213 | remove `crates/`; also: `monitoring/` → `compose/monitoring/` (lines 29, 33, 44, 50, 56, 62, 68, 79, 149, 151, 228) |
    | `docs/INFRASTRUCTURE.md` | 644, 647-648, 698, 700, 716, 773 | `nginx/` → `compose/nginx/`; `monitoring/` references on lines 52-56, 741-744 |
    | `docs/DEPLOY.md` | 57-58 | `nginx/` → `compose/nginx/` |
    | `docs/CI.md` | 184 | `e2e/` → `frontend/tests/e2e/` |
    | `CHANGELOG.md` | 36, 37, 73, 233 | `e2e/auth.test.ts`, `nginx/...`, `monitoring/...` → new paths |
    | `.github/.secrets.baseline` | 333, 336, 343 | `e2e/auth.test.ts` → `frontend/tests/e2e/auth.test.ts` |

### Verify
```bash
cd backend && cargo check
cd backend && cargo test --workspace
cd backend && cargo clippy -- -D warnings
cd frontend && bun test
```

---

## Idea 2: Template Installation

**Depends on Idea 1 being completed first** (install.sh references flattened crate paths).

Single `install.sh` at repo root that scaffolds a new project from this template.

### Usage
```bash
./install.sh my-new-project                    # full scaffold
./install.sh my-new-project --dry-run          # preview only
./install.sh my-new-project --skip-deps        # skip dependency install
./install.sh my-new-project --skip-git         # skip git init + commit
./install.sh my-new-project --skip-verify      # skip proof-of-concept build check
```

### Phases

#### Phase 1 — Validate
1. Reject missing/empty project name; reject if target directory already exists
2. Check required tools: Rust (1.95+), Bun (1.x+), uv (0.6+), Python 3.14+, Docker
3. If any tool missing, print install command and exit early

#### Phase 2 — Scaffold
4. `mkdir $PROJECT_NAME`
5. Copy trimmed copies (exclude `.git/`, `target/`, `node_modules/`, `.venv/`, `*.lock`, `dist/`):

   Directory copies:
   - `backend/` — Rust workspace (api, auth, cache, db, domain, py-sidecar, storage)
   - `compose/` — Docker Compose + Dockerfiles + nginx configs + monitoring (Prometheus/Grafana)
   - `frontend/` — Bun + React app + e2e tests (in `tests/e2e/`)
   - `python-sidecar/` → `$PROJECT_NAME/python-sidecar/` — Python FastAPI sidecar
   - `scripts/` — deploy, health, rollback, env utilities

   Root file copies:
   - `.env.example` `.gitignore` `.dockerignore` `Makefile`
   - `AGENTS.md` `CLAUDE.md` `CONTRIBUTING.md` `CODE_OF_CONDUCT.md` `LICENSE`

#### Phase 3 — Configure
6. Generate `.env` from `.env.example` with seeded `APP_NAME=$PROJECT_NAME`
7. Substitute `PROJECT_NAME` in template-aware files using `sed`:

   | File | What to replace |
   |------|-----------------|
   | `backend/Cargo.toml` | `repository` URL, `authors` |
   | `backend/*/Cargo.toml` | workspace metadata (inherited, so only root) |
   | `backend/py-sidecar/Cargo.toml` | `name` → keep as `py-sidecar` (crate name, not project-scoped) |
   | `frontend/package.json` | `name` field |
   | `python-sidecar/pyproject.toml` | `name` field |
    | `compose/prod.yml` | container names (`fullstackhex_` → `$PROJECT_NAME_`) |
    | `compose/dev.yml` | container names (`fullstackhex_` → `$PROJECT_NAME_`) |
    | `compose/monitor.yml` | container names (`fullstackhex_` → `$PROJECT_NAME_`) |
   | `compose/Dockerfile.rust` | crate pod paths (mirrors Idea 1 flattening) |
   | `Makefile` | `APP_NAME` variable at top |

#### Phase 4 — Install
8. `cd $PROJECT_NAME/python-sidecar && uv sync`
9. `cd $PROJECT_NAME/frontend && bun install`

#### Phase 5 — Verify (skippable)
10. `cd $PROJECT_NAME/backend && cargo check`
11. `cd $PROJECT_NAME/frontend && bun run typecheck`
12. Optional: `cd $PROJECT_NAME/python-sidecar && uv run pytest`

#### Phase 6 — Git
13. `cd $PROJECT_NAME && git init && git add . && git commit -m "chore: scaffold from fullstackhex template"`
14. Print directory tree + next-steps message

### Cleanup on Failure
- `trap` on ERR/EXIT: if `$PHASE < Configure`, remove `$PROJECT_NAME`
- If configure or later fails, print "partial scaffold left at $PROJECT_NAME — remove it and retry"

### Dry-Run Mode
- `--dry-run` prints every action without executing (mkdir, cp, sed, git, etc.)
- Validates args and tools but skips all mutations

### Files to Create
- `install.sh` (repo root)

### Files to Update
- `docs/INITIALIZATION.md` — reference install.sh
- `README.md` — add template usage section
