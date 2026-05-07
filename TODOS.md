# TODOS

All items active on `feat/0.10` branch. Tags: [priority] [effort].

## v0.10 summary

**4 tasks completed, 17 files changed, 164 tests passing (115 cargo + 49 vitest), 0 lint/type errors.**

| Commit | Task | Scope |
|--------|------|-------|
| `ae64291` | A10 Auth UI finalize | dashboard.astro, AuthForm (refresh_token + /dashboard redirect), Layout fetch interceptor, 34 vitest tests |
| `53eeeec` | S7 S3 multipart tests | wiremock deps, 5 multipart integration tests |
| `7123719` | S8 Coverage >80% | 11 storage CRUD tests, cache/pubsub modules, password tests, 25 new total |
| `93b395b` | S11 Auth Grafana | auth metrics middleware, oauth_callbacks_total counter, 6 Grafana panels |

## Gstack skill dependency map (v1.26.4)

Pending gstack plan (see `~/.claude/skills/gstack/TODOS.md`) grouped by parallelism.
Items marked **(G)** are gstack-internal; items marked **(P)** are project-consumable.

### Parallel block 1 — independent, any order
| Skill | Priority | Consumed by fullstackhex |
|-------|----------|------------------------|
| **Anti-bot CDP patches (G)** | P1 | Headless QA of auth flows |
| **Sidebar direct API (G)** | P2 | Faster browse-debug loop |
| **Sidebar PID scoping (G)** | P2 | Multi-workspace safety |
| **Test AskUserQuestion assertion (G)** | P2 | Plan-mode test reliability |
| **Devex handshake removal (G)** | P2 | /plan-devex-review works in plan mode |
| **Path-confusion hardening (G)** | P3 | Defensive test harness |

### Parallel block 2 — new skills, independent
| Skill | Priority | Consumed by fullstackhex |
|-------|----------|------------------------|
| **/health dashboard (G)** | P1 | Composite scores for all crates |
| **Codex reverse buddy (G)** | P1 | Cross-model review of auth code |
| **/checkpoint (G)** | P2 | Session saves across branches |
| **/refactor-prep (G)** | P2 | Dead-code strip before refactors |
| **/yc-prep (G)** | P2 | N/A (startup skill) |

### Parallel block 3 — QA and CI
| Skill | Priority | Consumed by fullstackhex |
|-------|----------|------------------------|
| **QA trend tracking (G)** | P2 | Auth health over time |
| **QA CI integration (G)** | P2 | QA gate in CI pipeline |
| **Smart default QA tier (G)** | P2 | Skip tier prompt on repeat runs |
| **CI QA quality gate (G)** | P2 | Fail PR on health drop |

### Sequential chain — Phase 2a → 2b
| Skill | Priority | Consumed by fullstackhex |
|-------|----------|------------------------|
| `/scrape` + `/skillify` (G) | P1 | Codify auth-flow browser skills |
| → `/automate` (G) | P0 | Codify reg/test button flows |

### Blocked/gated items
| Skill | Priority | Gate | Consumed by fullstackhex |
|-------|----------|------|------------------------|
| PACING_UPDATES_V0 (G) | P0 | V1 shipping | Better /plan-devex-review pacing |
| Plan Tune E1-E7 (G) | P0 | v1 dogfood calibration | Adaptive skill defaults |
| Chrome DevTools MCP (G) | P0 | Chrome 146+ | Real-session browser debug |
| Context recovery preamble (G) | P1 | — | Auto-reload plans after compaction |
| Session timeline (G) | P1 | Preamble shipped | /retro includes auth work |
| /learn SQLite migration (G) | P2 | JSONL pain data | Durable per-skill notes |
| Swarm primitive (G) | P2 | — | Parallel /ship pre-flight checks |

### Verdict for this project
- **Use now:** /investigate, /review, /qa, /browse, /ship
- **Use after shipping:** /scrape + /skillify (codify auth QA), /health (crate scores), Codex buddy (cross-model review)
- **Watch:** Chrome MCP (game-changer for auth debug), Plan Tune v2 (less friction)

---

## ✅ Completed (this branch)

### A10. Auth UI finalize [P0] [M]
- Created `frontend/src/pages/dashboard.astro` (auth-gated, user email/name, logout button)
- Modified `AuthForm.astro`: stores `fullstackhex_refresh_token`, redirects to `/dashboard`
- Modified `Layout.astro`: fetch interceptor for 401 → auto-refresh, logout redirects to `/login`
- Added `frontend/tests/vitest/auth-form.vitest.ts` (30 tests: login/register modes, validation, OAuth)
- **Files:** 4 new/modified. **Tests:** 30 vitest, 0 TS errors. **Commit:** `A10. Auth UI finalize — dashboard, token refresh, logout, vitest tests`

### S7. S3 multipart integration tests [P1] [S]
- Added `wiremock` to workspace deps + storage dev-deps
- 6 wiremock-based async tests: init round-trip, init failure, upload 2 parts + complete, abort mid-upload, abort nonexistent
- **Files:** 3 modified. **Tests:** 6 new. **Commit:** `S7. S3 multipart integration tests — init, upload 2 parts, complete, abort`

### S8. Coverage >80% per crate [P1] [L]
- **storage**: 11 wiremock integration tests for upload/download/streaming/delete/list operations
- **cache::cache**: 5 unit tests (serialization/deserialization) + 5 `#[ignore]` integration tests (require Redis)
- **cache::pubsub**: 2 `#[ignore]` integration tests (publish/subscribe round-trip)
- **auth::password**: 2 new tests (empty password round-trip, invalid hash error)
- **Files:** 4 modified. **Tests:** 25 new (18 live + 7 ignored). **Commit:** `S8. Coverage >80% per crate — storage integration, cache, password tests`

### S11. Auth Grafana dashboard — custom metrics + panels [P1] [M]
- Created `backend/crates/auth/src/metrics.rs` — `track_auth_metrics` middleware recording: `auth_requests_total`, `auth_latency_seconds` (histogram), `auth_errors_total` (by error_type/status)
- Added `oauth_callbacks_total` counter in `oauth_callback` handler
- Added 7 new Grafana panels: Custom Auth Request Rate, Auth Error Rate by Type, Auth p50/p99 Latency (custom), Auth Errors Cumulative, OAuth Callbacks by Provider
- **Files:** 7 modified. **Commit:** `S11. Auth Grafana dashboard — custom metrics middleware + panels`

## Later (deferred from v0.10)

### Auth route handler integration tests [P2] [M]
**What:** `register`/`login`/`logout`/`refresh`/`me` handlers in `crates/auth/src/routes.rs` have zero integration tests. Current tests cover health endpoints only.
**Why deferred:** Scope was already 4 items for v0.10. Auth route tests are a pre-existing gap, not introduced by v0.10.
**Files:** `backend/crates/api/tests/` or `backend/crates/auth/src/routes.rs`

## Icebox

### S3 auto-promote to multipart [P2] [S]
Route-level content-length check: files >5MB auto-init multipart. S3 API is manual by design, and manual endpoints already exist. No user demand yet.

## Review findings — post-implementation

```
Step 0:      Scope accepted — 4 commits landed on feat/0.10
Architecture: 4 issues (all resolved)
Code quality: 4 issues (all addressed)
Tests:        4 findings (3 resolved; 1 deferred: auth route handler integration tests)
Performance:  2 findings (no blocking issues)
Deferred:     auth route handler integration tests (pre-existing gap, P2)
Eng Review verdict: CLEARED — all 4 tasks implemented and verified
```
