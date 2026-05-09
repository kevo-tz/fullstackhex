# TODOS

## Goal: Simplify Makefile to 7 Dev-Only Commands

Strip the Makefile down to a thin dispatcher that delegates everything to `scripts/*.sh`. No production, deploy, migration, setup, or env management targets — just day-to-day dev.

---

## Target Commands

| Command       | Description                                        |
|---------------|----------------------------------------------------|
| `make dev`    | Start full stack (infra + python + rust + frontend) |
| `make down`   | Stop full stack and infrastructure                 |
| `make test`   | Run all test suites (rust + python + frontend)     |
| `make logs`   | Follow all stack logs                              |
| `make bench`  | Run performance benchmarks                         |
| `make status` | Show which services are running (PID, port, health)|
| `make clean`  | Reset to fresh state (removes volumes)             |

---

## Implementation

### Phase 1: Update `config.sh` — Add Missing Shared Variables

The Makefile currently defines several variables that the new scripts will need. Move them into `config.sh`:

- `PID_DIR` — `/tmp/fullstackhex-dev` (process ID directory)
- `PYTHON_SOCK` — `/tmp/fullstackhex-python.sock` (Unix socket for Python sidecar)
- `POSTGRES_RETRIES` — `6` (times to retry pg_isready)
- `POSTGRES_POLL_INTERVAL` — `5` (seconds between polls)
- `POST_START_DELAY` — `2` (seconds to wait after Rust start before health poll)
- `COMPOSE_DEV` — `docker compose -f compose/dev.yml --env-file .env`
- `COMPOSE_MON` — `docker compose -f compose/monitor.yml --env-file .env -p fullstackhex-monitor`

Insert the new vars before the export block, and add each to the export list.

Also add `load_env()` call to scripts that need `.env` values: `dev.sh`, `down.sh`, `clean.sh`.

---

### Phase 2: Create New Scripts

All new scripts follow this convention:

```bash
#!/bin/bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/config.sh"    # sources common.sh transitively
REPO_ROOT="$(get_repo_root)"
cd "$REPO_ROOT"                   # all paths are relative to repo root
trap cleanup EXIT INT TERM        # where applicable
```

#### 2a. `scripts/dev.sh` — Full Stack Startup

Replaces `run-dev` + `dev` + `watch` Makefile targets. Accepts optional `--watch` flag:

```bash
# Behavior:
#   default           cargo run -p api (background, nohup)
#   --watch           cargo watch -x 'run -p api' (background, nohup)

# Steps:
#   1. load_env (from common.sh) — sources .env from repo root
#   2. Check prerequisites: bun, uv, cargo, docker, docker compose
#   3. Preflight: check port 8001 (Rust), 4321 (Astro), PYTHON_SOCK not in use
#   4. mkdir -p PID_DIR, rm -f stale pidfiles and sock
#   5. docker compose -f compose/dev.yml up -d
#   6. Wait for PostgreSQL (pg_isready loop, max POSTGRES_RETRIES * POSTGRES_POLL_INTERVAL)
#   7. Start python sidecar: cd py-api && uv run uvicorn app.main:app --uds $PYTHON_SOCK
#   8. Start frontend: cd frontend && bun run dev
#   9. Start Rust backend (cargo run or cargo watch, nohup, log to PID_DIR/backend.log)
#  10. Sleep POST_START_DELAY seconds
#  11. Poll health: curl localhost:8001/health, /health/db, /health/python until OK or 30s timeout
#  12. Print dashboard URL (http://localhost:4321)
#  13. Wait on all background pids

# trap: cleanup calls down.sh, then exits
```

Signal handling: `trap cleanup EXIT INT TERM` where `cleanup()` calls `./scripts/down.sh` and `exit 1`. Trap must be set BEFORE starting any services, not at the end.

⚠️ **Known deviation from current Makefile**: The `verify-health` block previously ran as a Makefile prerequisite. Now it runs inline. Any health check failure will trigger `set -e` exit → `cleanup` trap → `down.sh`. This is correct behavior — don't leave a broken stack running.

#### 2b. `scripts/down.sh` — Stop Everything

Replaces `down-dev` Makefile target.

```bash
# Steps:
#   1. load_env (gets PYTHON_SOCK from .env if set)
#   2. Iterate PID_DIR/*.pid, kill each PID, rm pidfile
#   3. Safety pkill -x uvicorn, api, bun (orphaned processes — only if PID exists)
#   4. docker compose -f compose/dev.yml down
#   5. docker compose -f compose/monitor.yml down -p fullstackhex-monitor
#   6. rm -f $PYTHON_SOCK
```

**Idempotent**: Safe to call multiple times. If nothing is running, it's a no-op.

#### 2c. `scripts/logs.sh` — Follow All Logs

Replaces `logs-*` Makefile targets.

```bash
# Steps:
#   1. [ -f "$PID_DIR/backend.log" ] && tail -f "$PID_DIR/backend.log" & (with label prefix)
#   2. docker compose -f compose/dev.yml logs -f postgres redis & (in background)
#   3. Print: "Python and frontend logs appear in their terminal windows"
#   4. trap INT → kill all background jobs, exit
#   5. wait
```

⚠️ `set -euo pipefail` must NOT be used here — `tail -f` on a missing file should warn, not abort. Use explicit checks:
```bash
[ -f "$PID_DIR/backend.log" ] && tail -f "$PID_DIR/backend.log" || log_warning "Backend log not found (start dev first)"
```

#### 2d. `scripts/test.sh` — Run All Tests

Replaces `test`, `test-rust`, `test-python`, `test-frontend` Makefile targets.

⚠️ `set -e` must NOT be used — need to run all four suites even if one fails:

```bash
# Steps:
#   EXIT=0
#   cd backend && cargo test --workspace || EXIT=$?
#   cd "$REPO_ROOT" && cd py-api && uv run pytest || EXIT=$?
#   cd "$REPO_ROOT" && cd frontend && bun test || EXIT=$?
#   cd "$REPO_ROOT" && cd frontend && bun run test:vitest || EXIT=$?
#   exit $EXIT
```

Uses `|| EXIT=$?` pattern to accumulate exit codes. Each suite always runs.

Individual test suites remain runnable directly (`cargo test`, `bun test`, etc.) — no need to proxy them through Makefile.

#### 2e. `scripts/clean.sh` — Reset to Fresh State

Replaces `clean` Makefile target.

```bash
# Steps:
#   1. load_env (gets PYTHON_SOCK)
#   2. docker compose -f compose/dev.yml down -v --remove-orphans
#   3. docker compose -f compose/monitor.yml down -v --remove-orphans
#   4. rm -rf "$PID_DIR"
#   5. rm -f "$PYTHON_SOCK"
```

---

### Phase 3: Rewrite Makefile

Replace the entire Makefile (~470 lines) with:

```makefile
.PHONY: dev down test logs bench status clean

help:
	@echo "Usage: make [dev|down|test|logs|bench|status|clean]"
	@echo ""
	@echo "  dev      Start full stack (infra + apps)"
	@echo "  down     Stop all services"
	@echo "  test     Run all test suites"
	@echo "  logs     Follow all stack logs"
	@echo "  bench    Run performance benchmarks"
	@echo "  status   Show service status (PID, port, health)"
	@echo "  clean    Reset to fresh state (removes volumes)"
	@echo ""
	@echo "Quick start: make dev"
	@echo "          → http://localhost:4321"

.DEFAULT_GOAL := help

dev:
	@./scripts/dev.sh

down:
	@./scripts/down.sh

test:
	@./scripts/test.sh

logs:
	@./scripts/logs.sh

bench:
	@./scripts/bench.sh

status:
	@./scripts/status.sh

clean:
	@./scripts/clean.sh
```

40 lines. 7 targets. Everything else is in `scripts/`.

---

### Phase 4: Update Existing Scripts

#### 4a. `scripts/status.sh`

Currently self-contained with its own `PID_DIR` default. Source `config.sh` for consistency:
```bash
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/config.sh"
```
Then remove the inline `PID_DIR="${PID_DIR:-/tmp/fullstackhex-dev}"` — it's now inherited from config.

#### 4b. `scripts/bench.sh`

No changes needed. Already sources `config.sh` and `common.sh`.

#### 4c. `scripts/common.sh`

The test helpers (`assert_*`, `mock_*`, test lifecycle) occupy ~180 lines and are only used by `test_example.sh` (which is being deleted). Trim them to keep `common.sh` focused on operational utilities.

Remove these functions (lines ~320-487):
- `test_mode()` / `mock_command()` / `mock_read_file()` / `mock_write_file()` / `mock_env()` / `mock_network_calls()`
- `assert_equals()` / `assert_contains()` / `assert_file_exists()` / `assert_command_exists()` / `assert_not_contains()` / `assert_exit_code()`

---

### Phase 5: Delete Unused Scripts

| Script | Deletes? | Reason |
|--------|----------|--------|
| `scripts/install-deps.sh` | yes | One-time setup, not daily dev |
| `scripts/setup-env.sh` | yes | One-time setup |
| `scripts/validate-env.sh` | yes | Absorbed into `dev.sh` (inline check) |
| `scripts/sync-env.sh` | yes | Manual utility, not daily dev |
| `scripts/verify-health.sh` | yes | Absorbed into `dev.sh` |
| `scripts/contract-test.sh` | yes | Absorbed into `test.sh` |
| `scripts/rollback.sh` | yes | Deploy tooling |
| `scripts/deploy-blue-green.sh` | yes | Deploy tooling |
| `scripts/deploy-canary.sh` | yes | Deploy tooling |
| `scripts/deploy-canary-promote.sh` | yes | Deploy tooling |
| `scripts/deploy-canary-rollback.sh` | yes | Deploy tooling |
| `scripts/deploy-verify.sh` | yes | Deploy tooling |
| `scripts/baseline.sh` | yes | Standalone profiler — `bench.sh` doesn't support `--save` yet |
| `scripts/test_example.sh` | yes | Test framework demo, not used |
| `scripts/test/helpers.sh` | yes | Only used by `test_example.sh` |
| `scripts/test/` | yes | Entire directory — empty after helpers.sh removal |

---

### Phase 6: Update Documentation

| File | What to update |
|------|----------------|
| `README.md` | Command reference — remove `make setup`, `make up`, `make deploy`. Replace with 7 dev targets |
| `CLAUDE.md` | Health stack section already uses direct commands — verify no stale Makefile refs |
| `AGENTS.md` | Verify no stale Makefile target refs in skill routing |
| `docs/SETUP.md` | Replace `make setup` with `bun install`, `cp .env.example .env`, `cd backend && cargo build` |
| `docs/CI.md` | Replace `make test-rust/test-python/test-frontend` with direct commands |
| `docs/SERVICES.md` | Replace `make logs-backend/logs-frontend` with `make logs` |
| `docs/EXAMPLES.md` | Replace `make migrate` with `cd backend && cargo sqlx migrate run` |
| `docs/MONITORING.md` | Replace `make up` with `docker compose -f compose/dev.yml up -d` |
| `docs/INFRASTRUCTURE.md` | Replace `make deploy/deploy-check` with direct commands |
| `docs/DEPLOY.md` | Deprecate or move to `docs/archive/` — deployment docs go with deleted scripts |
| `docs/INITIALIZATION.md` | Replace `make setup` with direct commands |

---

### Phase 7: Verify

```bash
shellcheck scripts/*.sh

make              # shows help text (default goal)

make dev          # full stack starts, dashboard printed
make status       # all services green with PIDs
make down         # clean shutdown
make test         # all 4 suites run, non-zero if any fail
make logs         # log output visible (Ctrl+C to exit)
make bench        # benchmarks run (requires ab)
make clean        # volumes removed
```

---

## Execution Order

```
Phase 1:  Edit config.sh              add PID_DIR, PYTHON_SOCK, compose vars + exports
Phase 2:  Create scripts              dev.sh, down.sh, logs.sh, test.sh, clean.sh
Phase 3:  Rewrite Makefile            7-target dispatcher with help/default
Phase 4:  Update existing scripts     status.sh (source config), common.sh (trim test helpers)
Phase 5:  Delete unused scripts/test  rm 15 files + scripts/test/ dir
Phase 6:  Update docs                 README, CLAUDE.md, docs/*.md
Phase 7:  Verify                      shellcheck + manual smoke test all 7 targets
```

**Checkpoint after Phase 3**: All 7 `make` targets work. Phases 4-7 are cleanup.

---

## Key Traps to Avoid During Implementation

| Trap | Why | Fix |
|------|-----|-----|
| `set -e` in `test.sh` | Kills on first suite failure, never runs remaining suites | Use `|| EXIT=$?` pattern instead |
| `set -e` in `log.sh` | `tail -f` on missing file aborts script | Explicit `[ -f ... ]` check before tail |
| `load_env()` not called | `PYTHON_SOCK` remains empty, compose may not find `.env` | Call `load_env` in dev.sh, down.sh, clean.sh |
| Wrong working directory | Scripts in `scripts/` run docker compose with relative paths | `cd "$REPO_ROOT"` after sourcing config |
| `nohup` missing for Rust backend | Rust dies when shell exits | Use `nohup cargo ... > "$PID_DIR/backend.log" 2>&1 &` |
| `pkill -x uvicorn` kills test sidecar | Broad name match | Gate pkill on PID existence, or use `pidof` |
| `config.sh` sources `common.sh` then script sources both | Redundant but harmless | Just keep both — no need to refactor |
| `SIGINT` during startup leaves partial stack | Trap set too late | Set `trap cleanup EXIT INT TERM` as first action after sourcing |

---

## Rollback

If something breaks:

1. `git checkout -- Makefile` — restores original Makefile
2. `rm scripts/dev.sh scripts/down.sh scripts/logs.sh scripts/test.sh scripts/clean.sh` — delete new scripts
3. `git checkout -- scripts/config.sh scripts/status.sh scripts/common.sh` — restore modified scripts
4. `git checkout -- scripts/<deleted_file>` — restore individual deleted scripts as needed
