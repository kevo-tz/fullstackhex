# TODOS

## Completed

- Wire PythonSidecar IPC + real DB health checks (v0.3.1.0)
- Parallelize frontend health fetches (v0.4.x)
- Add `make watch` target (v0.4.x)
- Document log locations per service (v0.4.x)
- Fix outdated Python sidecar docs in SERVICES.md (v0.4.x)
- Add `make logs-python` target (v0.4.x)

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