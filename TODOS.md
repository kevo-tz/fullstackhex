# FullStackHex - Improvement Plan

## Project Analysis Summary

### Documentation vs. Implementation Alignment
**Status: ✓ ALIGNED**

- install.sh creates exact artifact structure promised in SETUP.md
- Crate layout (api, core, db, python-sidecar) matches SERVICES.md  
- Environment variables match INFRASTRUCTURE.md specs
- Docker Compose files reference correct networks and volumes
- Test scaffolding aligns with performance-budget.md

### Dry-Run Results

**Test Environment**: Linux, Python 3.12.3, Rust 1.95.0, Bun 1.3.13, Docker present

**Script Behavior**: 
- install.sh correctly validates Python 3.14+ requirement (exits with helpful error)
- Proper error handling for missing Python version
- Scripts are idempotent (safe to re-run)
- Color output and progress indicators work correctly

---

## Critical Issues

### 🔴 P1: Production Compose YAML Is Invalid
**File**: compose/prod.yml, lines ~39-44
**Issue**: The `nginx.depends_on` block is malformed; `backend:` is misindented and breaks YAML parsing
**Impact**: `docker compose -f compose/prod.yml config` fails before any build or startup work begins
**Fix**: Repair the `depends_on` structure under `nginx` before validating the rest of the production stack
**Estimated Effort**: 15 mins

### 🔴 P1: Production Compose Build Contexts Are Incorrect
**File**: compose/prod.yml, lines ~64, ~109, ~136
**Issue**: `build.context: .` resolves relative to `compose/prod.yml`, so Docker builds run from `./compose` instead of the repo root
**Impact**: Backend, frontend, and python-sidecar images cannot reliably copy source files from `backend/`, `frontend/`, or `python-sidecar/`
**Fix**: Keep production Dockerfiles inside `./compose`, but update each build to use the repository root as context (for example `context: ..`) while referencing compose-local Dockerfiles explicitly
**Estimated Effort**: 45 mins

### 🔴 P1: Missing Compose-Local Dockerfiles
**File**: compose/prod.yml, lines ~66, ~111, ~138
**Issue**: prod.yml references `Dockerfile.rust`, `Dockerfile.python`, and `Dockerfile.frontend`, but those files do not exist yet under `./compose`
**Impact**: Production image builds fail before the stack can start
**Fix**: Create `compose/Dockerfile.rust`, `compose/Dockerfile.python`, and `compose/Dockerfile.frontend`
**Estimated Effort**: 4 hours

```dockerfile
# Suggested structure:
# Stage 1: Builder (compile app from repo-root build context)
# Stage 2: Runtime (minimal image, copy only runtime artifacts)
```

### 🔴 P1: Missing Compose-Local Nginx Assets
**File**: compose/prod.yml references `./nginx/nginx.conf`, `./nginx/certs/`
**Issue**: `compose/nginx/nginx.conf` and the expected `compose/nginx/certs/` layout are not present yet
**Impact**: Production proxy layer will not start; TLS termination remains undefined
**Fix**: Create `compose/nginx/nginx.conf` with Rust/Astro routing rules and document the expected certificate layout under `compose/nginx/certs/`
**Estimated Effort**: 2 hours

### 🟠 P2: Python 3.14 Requirement Too Strict
**File**: scripts/install.sh, lines ~65-75
**Issue**: Python 3.14 may not be available in all dev environments; script exits immediately
**Current State**: Requires manual pyenv installation (friction point for new users)
**Suggestion**: Add `--skip-python` flag to allow experienced users to use existing Python 3.13+
**Alternative**: Document fallback to Python 3.13 with clear caveats about breaking changes
**Estimated Effort**: 1.5 hours

### 🟠 P2: Socket Path Not Configurable
**File**: install.sh (line ~200), multiple docs
**Issue**: Unix socket hardcoded to `/tmp/python-sidecar.sock` on all systems
**Risk**: On shared/multi-user systems, `/tmp` is world-readable; socket could be exploited
**Suggestion**: 
  1. Add `PYTHON_SIDECAR_SOCKET` to .env.example (with default)
  2. Allow override via environment variable
  3. Default to `~/.fullstackhex/sockets/` for user isolation
**Estimated Effort**: 1 hour

---

## Important Issues

### 🟡 P3: bench.sh Missing Service Pre-flight Check
**File**: scripts/bench.sh, line ~30 (check_deps)
**Issue**: Script checks for `bombardier` binary but not if services are actually running
**Impact**: Script fails with confusing error if Rust backend or frontend aren't up
**Fix**: Add `curl --silent http://localhost:8001/health` + `curl --silent http://localhost:4321` before benchmarks
**Suggested Error Message**:
```
✗ Rust backend not responding at http://localhost:8001
  Run: cd backend && cargo run -p api
✗ Frontend not responding at http://localhost:4321  
  Run: cd frontend && bun run dev
```
**Estimated Effort**: 45 mins

### 🟡 P3: bench.sh Requires External Tool (Go)
**File**: scripts/bench.sh, performance-budget.md
**Issue**: bombardier requires Go installation; adds extra dependency for benchmarking
**Alternative**: Use `ab` (Apache Bench, pre-installed) or implement in Rust/Python
**Suggestion**: Create `scripts/bench-lite.sh` using `ab` with same performance targets
**Estimated Effort**: 1.5 hours

### 🟡 P3: PATH Handling for Bun Installation
**File**: scripts/install.sh, lines ~45-61
**Issue**: Bun installation relies on sourcing a small set of shell rc files and may still leave `bun` off PATH in some environments
**Impact**: Fresh installs can succeed but leave the next script step unable to find `bun`
**Fix**: Detect the active shell more reliably and emit an explicit PATH export/install hint when Bun is installed into `~/.bun/bin`
**Estimated Effort**: 1 hour

### 🟡 P3: Missing Monitoring Configuration Files
**Files**: compose/monitor.yml references `./monitoring/prometheus.yml`, `./monitoring/grafana/provisioning/`, `./monitoring/grafana/dashboards/`
**Issue**: All three paths are referenced in compose/monitor.yml (lines 24, 53-54) and described in docs/INFRASTRUCTURE.md, but none of those files or directories exist in the repository
**Impact**: `docker compose -f compose/monitor.yml up -d` fails immediately; the monitoring overlay is completely non-functional despite being documented as a supported feature
**Fix**: Create the missing monitoring configuration files:
  - `monitoring/prometheus.yml` — scrape configs for Rust backend (:8001/metrics) and node-exporter
  - `monitoring/grafana/provisioning/datasources/prometheus.yml` — Prometheus datasource definition
  - `monitoring/grafana/provisioning/dashboards/dashboards.yml` — dashboard discovery config
  - `monitoring/grafana/dashboards/overview.json` — starter dashboard (p99 latency, error rates, RPS)
**Estimated Effort**: 2 hours

### 🟡 P3: CI Workflow Socket Path May Fail on Restricted Runners
**File**: .github/workflows/ci.yml, install.sh (line ~470)
**Issue**: `install.sh` hardcodes `PYTHON_SIDECAR_SOCKET=/tmp/python-sidecar.sock` into `.env`. On some GitHub Actions runners and security-hardened CI environments `/tmp` socket binding is restricted or `/tmp` is not writable by the runner user
**Impact**: Generated smoke tests and any CI job that exercises the socket path may silently skip or error without a clear message pointing to the socket
**Fix**:
  1. In `install.sh`, detect `CI=true` (set automatically by GitHub Actions) and default the socket to `$RUNNER_TEMP/python-sidecar.sock` or a project-local path such as `$PWD/.tmp/python-sidecar.sock`
  2. Document this behaviour in the CI section of the README / a future docs/CI.md
**Estimated Effort**: 45 mins

---

## Documentation Improvements

### 🟡 P3: ARCHITECTURE.md Missing IPC Implementation Details
**File**: docs/ARCHITECTURE.md
**Issue**: Shows Unix socket connection but no details on Rust crate implementation
**Suggestion**: Add code examples showing:
  - How Rust crate spawns Python subprocess
  - How to send HTTP requests over Unix socket using hyper
  - Error handling for socket failures
**Estimated Effort**: 1.5 hours

### 🟢 P4: performance-budget.md References Missing Benchmarks
**File**: docs/performance-budget.md, performance table
**Issue**: Several benchmarks listed (DB query <10ms, hot path allocs) but no CI gate implementation shown
**Suggestion**: Show example SQL benchmark and flamegraph commands
**Estimated Effort**: 1 hour

---

## Enhancement Opportunities

### 🟢 P4: Add Health Check Verification Script
**New File**: scripts/verify-health.sh
**Purpose**: Check all services are healthy before running tests/benchmarks
**What It Should Do**:
  - Poll `/health` endpoints with configurable timeout
  - Report which services are up/down
  - Suggest docker-compose commands if services aren't running
  - Return exit code 0 only if all healthy
**Estimated Effort**: 1.5 hours

### 🟢 P4: Generate Baseline Performance Profile
**New File**: scripts/baseline.sh
**Purpose**: Capture initial performance snapshot for regression detection
**What It Should Do**:
  - Run bench.sh and save results to JSON
  - Commit baseline to git for CI comparison
  - Generate HTML report
**Estimated Effort**: 2 hours

### 🟢 P4: Add Monitoring Stack Documentation
**New File**: docs/MONITORING.md
**Issue**: compose/monitor.yml exists but no guide on how to use Prometheus/Grafana
**What It Should Include**:
  - How to access Grafana at :3000
  - Pre-configured dashboards for FullStackHex stack
  - Key metrics to watch (p99 latency, error rates, etc.)
  - How to set up alerts
**Estimated Effort**: 2 hours

### 🟢 P4: Local Development Quick Commands
**New File**: Makefile or scripts/dev-commands.sh
**Purpose**: Reduce cognitive load of command sequences
**Suggested Commands**:
```bash
make up          # Start all services
make down        # Stop all services  
make logs-backend # Follow Rust backend logs
make logs-frontend # Follow Astro logs
make test        # Run all test suites
make bench       # Run performance benchmarks
make clean       # Reset to fresh state
```
**Estimated Effort**: 1 hour

### 🟢 P4: Add CI/CD Pipeline Documentation
**Enhancement**: docs/CI.md
**Purpose**: Explain GitHub Actions pipeline referenced but not documented
**Include**:
  - Required secrets for CI
  - Performance gate criteria
  - How to debug failing checks locally
  - Dependency update process
**Estimated Effort**: 1.5 hours

### 🟢 P4: Document compose/ Directory Structure
**Files**: docs/INFRASTRUCTURE.md, docs/SETUP.md
**Issue**: Neither doc explains the expected layout *inside* the `compose/` directory. New contributors have no guidance on where to place Dockerfiles, the nginx config, nginx certs, or monitoring configs relative to the compose files
**Impact**: Developers creating the missing production assets (Dockerfiles, nginx.conf, monitoring configs) are likely to place them at the wrong path, causing silent volume-mount failures
**Fix**: Add a `compose/ Directory Layout` section to docs/INFRASTRUCTURE.md covering:
  - `compose/Dockerfile.rust`, `compose/Dockerfile.python`, `compose/Dockerfile.frontend`
  - `compose/nginx/nginx.conf` and `compose/nginx/certs/{fullchain,privkey}.pem`
  - `compose/` vs repo-root build context distinction
**Estimated Effort**: 30 mins

---

## Testing Gaps

### 🟢 P4: Generated Tests Need Real Assertions
**Files**: All test files created by install.sh
**Issue**: Generated tests are placeholders (e.g., `assert_eq!(2+2, 4)`)
**Suggestion**: Replace with meaningful stubs:
  - Rust: Health endpoint returns 200 with expected JSON
  - Python: Sidecar app starts without errors
  - Frontend: Health proxy route is accessible
**Estimated Effort**: 1.5 hours

### 🟢 P4: Missing Integration Test Template
**New File**: backend/crates/api/tests/integration_socket.rs
**Purpose**: Test Unix socket communication between Rust and Python
**What It Should Test**:
  - Socket connection establishment
  - Request forwarding to Python sidecar
  - Error handling for socket failures
**Estimated Effort**: 2 hours

---

## Priority Breakdown

| Priority | Count | Est. Hours | Impact |
|----------|-------|-----------|--------|
| P1 (Critical) | 4 | 7.0 | Production deployment broken |
| P2 (Important) | 2 | 2.5 | Developer experience friction |
| P3 (Should) | 7 | 9.25 | Functionality/usability gaps |
| P4 (Nice) | 7 | 11.5 | Automation/polish |
| **Total** | **20** | **30.25** | |

---

## Recommended Implementation Order

### Phase 1 (Unblock Production) - 7 hours
1. Repair the malformed `nginx.depends_on` block in `compose/prod.yml`
2. Update `compose/prod.yml` build contexts to use the repo root while keeping Dockerfiles under `./compose`
3. Create `compose/Dockerfile.rust`
4. Create `compose/Dockerfile.python`
5. Create `compose/Dockerfile.frontend`
6. Create `compose/nginx/nginx.conf` and define `compose/nginx/certs/` expectations
7. Test that `docker compose -f compose/prod.yml config` and the production builds succeed

### Phase 2 (Improve DX) - 2.5 hours
8. Add `--skip-python` flag to install.sh OR document Python 3.13 fallback
9. Make socket path configurable via .env

### Phase 3 (Complete Documentation) - 9.25 hours
10. Fix bench.sh service pre-flight check
11. Expand ARCHITECTURE.md with code examples
12. Add scripts/verify-health.sh utility
13. Create monitoring configuration files (prometheus.yml, Grafana provisioning, starter dashboard)
14. Fix CI socket path handling in install.sh for `CI=true` environments

### Phase 4 (Polish & Automation) - 11.5 hours
15. Add Makefile for common dev commands
16. Improve generated test assertions
17. Add CI/CD documentation
18. Create monitoring guide
19. Implement baseline performance profiling
20. Add Unix socket integration tests
21. Document compose/ directory layout in INFRASTRUCTURE.md

---

## Verification Checklist

After implementing improvements:

- [ ] `docker compose -f compose/prod.yml config` parses cleanly and resolves without path errors
- [ ] `docker compose -f compose/prod.yml build` succeeds using Dockerfiles under `compose/`
- [ ] `compose/prod.yml up -d` succeeds without errors
- [ ] `docker compose -f compose/dev.yml up -d` + `docker compose -f compose/monitor.yml up -d` fully operational
- [ ] `monitoring/prometheus.yml`, Grafana provisioning, and starter dashboard all present and loaded by monitor stack
- [ ] `./scripts/install.sh` completes on Python 3.13+ systems (with warning)
- [ ] `CI=true ./scripts/install.sh` uses a non-`/tmp` socket path
- [ ] `./scripts/verify-health.sh` reports all services healthy
- [ ] `./scripts/bench.sh` runs and produces performance report
- [ ] Documentation additions are completed and cross-linked
- [ ] Test suites have meaningful assertions (not placeholder values)
- [ ] Makefile/dev commands reduce typing for common workflows

---

## Notes

**Strengths Observed**:
- Clear architectural decisions with good documentation of rationale
- Idempotent scripts that are safe to re-run
- Comprehensive .env.example covering all services
- Good use of Docker Compose for local dev
- Color-coded output in scripts aids readability

**Design Philosophy Alignment**:
- "Latest tooling" promise is well-executed (Rust 2024, Astro 5+, etc.)
- One-command init works (with Python 3.14+ caveat)
- Unix socket choice is pragmatic for IPC performance

