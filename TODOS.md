# TODOS

## Later (all done ✅)

### ✅ A8. Add `make test-e2e` [P1] [L]
Bun-based e2e auth tests (`e2e/auth.test.ts`), `make test-e2e` target, CI e2e job with postgres+redis services.

### ✅ S7. Multipart upload for files > 5MB [P2] [L]
Multipart upload APIs: init, upload part, complete, abort. Routes at `POST /storage/multipart/init`, etc.

### ✅ S10. End-to-end shell test [P2] [L]
Full user journey shell test (`tests/e2e.sh`): health → register → login → /auth/me → upload → download → delete → dashboard. `make test-e2e-shell` target.

## Icebox

### S9. bats-core tests for deploy scripts [P2] [M]
**What:** Deploy safety scripts are shell scripts with no automated tests.
**Fix:** Add `tests/deploy/` with bats-core tests. Mock docker compose, nginx, scp, `.deploy-state` file.
**Files:** `tests/deploy/`, `scripts/deploy-*.sh`
**Trigger:** CI starts running deploy scripts

### Run ignored socket tests in CI [P2] [M]
**What:** Start test FastAPI instance as CI background step so `#[ignore]` socket integration tests run automatically.
**Why not now:** Socket tests pass on native Linux but fail on WSL2 due to Unix socket quirks. May be flaky in CI.
**Files:** `python-sidecar/src/lib.rs`, `integration_health_route.rs`
**Trigger:** WSL2 CI support or native Linux CI runner

### Add inline Rust doc examples [P2] [S]
**What:** `///` doc comments on `PythonSidecar::get()`, `PythonSidecar::health()`, and `db::health_check` with usage examples.
**Why not now:** Doc examples can rot if not compiled. Low priority for solo dev.
**Trigger:** First external contributor or user request

### Add concrete examples to docs [P2] [S]
**What:** New `docs/EXAMPLES.md` with copy-paste code blocks showing how to extend the template.
**Why not now:** Examples must be maintained as API evolves. Templates change quickly in v0.x.
**Trigger:** First external contributor or stable v1.0 API
