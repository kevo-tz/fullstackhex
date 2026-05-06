# TODOS

## Later (all done ✅)

### ✅ A8. Add `make test-e2e` [P1] [L]
Bun-based e2e auth tests (`e2e/auth.test.ts`), `make test-e2e` target, CI e2e job with postgres+redis services.

### ✅ S7. Multipart upload for files > 5MB [P2] [L]
Multipart upload APIs: init, upload part, complete, abort. Routes at `POST /storage/multipart/init`, etc.

### ✅ S10. End-to-end shell test [P2] [L]
Full user journey shell test (`tests/e2e.sh`): health → register → login → /auth/me → upload → download → delete → dashboard. `make test-e2e-shell` target.

## Icebox (all done ✅)

### ✅ S9. bats-core tests for deploy scripts [P2] [M]
`tests/deploy/deploy_scripts.bats` — 18 bats tests covering rollback, blue-green, canary, verify scripts. Mocks ssh, scp, rsync, docker, nginx, flock.

### ✅ Run ignored socket tests in CI [P2] [M]
CI rust job now runs `cargo test -p python-sidecar -- --ignored` with Python sidecar running on Unix socket.

### ✅ Add inline Rust doc examples [P2] [S]
`///` doc examples on `PythonSidecar::get()`, `PythonSidecar::health()`, and `db::health_check`. `cargo test --doc` passes.

### ✅ Add concrete examples to docs [P2] [S]
`docs/EXAMPLES.md` — copy-paste code blocks for new routes, pages, migrations, storage ops, cache, Grafana panels, sidecar calls, e2e tests, and CI checks.

---

All items shipped. No remaining TODOs.
