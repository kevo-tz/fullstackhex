# TODOS

## Completed

- Wire PythonSidecar IPC + real DB health checks (v0.3.1.0)

## Deferred

### Run ignored socket tests in CI
**What:** Start a test FastAPI instance as a CI background step so the 5 `#[ignore]` socket integration tests run automatically.
**Why:** Socket tests never run — they require `--ignored` flag. CI has Python setup but doesn't start a sidecar.
**Pros:** Catches socket regressions automatically. Closes the test coverage gap on the polyglot claim.
**Cons:** Adds ~30s to CI runs. Socket tests are timing-sensitive and may be flaky in CI.
**Context:** Tests in `python-sidecar/src/lib.rs` (4 ignored) and `integration_health_route.rs` (1 ignored). All use mock UnixListener. They pass on native Linux but fail on WSL2 due to Unix socket quirks.
**Depends on:** CI Python setup (already exists).

### Parallelize frontend health fetches
**What:** Change `health.ts` from sequential `await fetch()` to `Promise.allSettled()`.
**Why:** With PythonSidecar timeout at 5s, dashboard takes 5-15s to update all dots. Parallel fetches: ~5s max.
**Pros:** Immediate UX improvement. ~5 line change. Dashboard dots update simultaneously.
**Cons:** None — each fetch already has its own try/catch.
**Context:** `frontend/src/pages/api/health.ts:14-33`. Design doc deferred this from v0.4.
**Depends on:** —

### Add `make watch` target
**What:** Target that starts Docker infra + Python sidecar + `cargo watch -x run` + `bun dev`.
**Why:** Rust backend has no hot reload. Bun and Python have it. Rust devs expect `cargo watch`.
**Pros:** Fast iteration across all 3 languages. One command to start dev mode with live reload.
**Cons:** cargo watch adds a dependency (`cargo install cargo-watch`). Three terminal streams interleave.
**Context:** Makefile `dev` target already starts everything. `watch` is the same but with cargo watch.
**Depends on:** —

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

### Document log locations per service
**What:** Section in `docs/SERVICES.md` listing where to find logs for each service.
**Why:** Debugging requires checking 3 separate processes with different log outputs.
**Pros:** Single reference point for "where are my logs?" — common pain point.
**Cons:** Low effort, just documentation.
**Depends on:** —