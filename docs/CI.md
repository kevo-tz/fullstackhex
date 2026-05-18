# CI/CD Pipeline Documentation

## Overview

FullStackHex uses GitHub Actions for continuous integration and deployment. The pipeline is defined in `.github/workflows/ci.yml`.

## Pipeline Stages

### 1. Lint and Format Check
- **Rust**: `cargo fmt --check` and `cargo clippy`
- **TypeScript/Frontend**: `bun run lint` (if configured)
- **Python**: `ruff check` (if configured)

### 2. Build
- **Rust**: `cargo build --workspace`
- **Frontend**: `bun run build`
- **Python**: `uv sync --all-extras`

### 3. Test
- **Rust**: `cargo test --workspace`
- **Frontend**: `vitest run`
- **Python**: `pytest`

### 4. Performance Gates (Optional)
- Runs `scripts/bench.sh` if `RUN_BENCHMARKS=true` is set
- Checks p50 < 5ms, p99 < 20ms for /health endpoint
- TTFB < 0.1s for frontend

## Required Secrets

Configure these in GitHub repository settings (**Settings → Secrets and variables → Actions**):

| Secret | Description | Required For |
|--------|-------------|--------------|
| `POSTGRES_PASSWORD` | Database password | Tests |
| `REDIS_PASSWORD` | Redis password | Tests |
| `RUSTFS_ACCESS_KEY` | S3-compatible storage access key | Integration tests |
| `RUSTFS_SECRET_KEY` | S3-compatible storage secret key | Integration tests |

## Environment Variables

These are set in `.github/workflows/ci.yml` or repository variables:

| Variable | Default | Description |
|----------|---------|-------------|
| `RUST_LOG` | `info` | Rust logging level |
| `PYTHON_LOG_LEVEL` | `info` | Python logging level |
| `RUN_BENCHMARKS` | `false` | Enable performance benchmarks |
| `CI` | `true` | Auto-set by GitHub Actions |

## Performance Gate Criteria

Performance gates are configured in `scripts/bench.sh` and `docs/performance-budget.md`:

```yaml
# Example performance check in CI
- name: Run Benchmarks
  if: env.RUN_BENCHMARKS == 'true'
  run: |
    ./scripts/bench.sh
    # Fails CI if benchmarks don't meet targets
```

### Targets

| Metric | Target | Endpoint |
|--------|--------|----------|
| p50 Latency | < 5ms | `/health` |
| p99 Latency | < 20ms | `/health` |
| Frontend TTFB | < 0.1s | `/` |
| RPS | > 1000 | `/health` |

## Debugging Failing Checks Locally

### Lint Issues

```bash
# Rust
cd backend
cargo fmt --check  # Check formatting
cargo clippy -- -D warnings  # Lint with warnings as errors

# Python
cd py-api
ruff check .

# Frontend
cd frontend
bun run lint  # If configured
```

### Test Failures

```bash
# Run all tests
make test

# Or individually
cd backend && cargo test
cd py-api && uv run pytest
cd frontend && vitest run

# Run with verbose output
cd backend && cargo test -- --nocapture
cd py-api && uv run pytest -v
cd frontend && vitest run --reporter verbose
```

### Socket Path Issues in CI

The `PYTHON_SIDECAR_SOCKET` env var must point to a writable path. In CI, set it to a temp path:

```bash
# In CI (e.g. GitHub Actions), set the socket to a temp directory
export PYTHON_SIDECAR_SOCKET="${RUNNER_TEMP}/py-api.sock"
```

To debug socket issues locally:

```bash
make status
```

### Performance Issues

```bash
# Run benchmarks locally
./scripts/bench.sh

# Check specific endpoints
curl -w "@curl-format.txt" http://localhost:8001/health
# Where curl-format.txt contains:
#      time_namelookup: %{time_namelookup}\n
#         time_connect: %{time_connect}\n
#      time_appconnect: %{time_appconnect}\n
#     time_pretransfer: %{time_pretransfer}\n
#        time_redirect: %{time_redirect}\n
#   time_starttransfer: %{time_starttransfer}\n
#                     -------\n
#          time_total: %{time_total}\n
```

## Dependency Update Process

### Rust Dependencies

```bash
cd backend
cargo update              # Update all dependencies
cargo update -p tokio   # Update specific package
```

### Python Dependencies

```bash
cd py-api
uv sync --all-extras
uv lock
```

### Frontend Dependencies

```bash
cd frontend
bun update                 # Update all packages
bun update react            # Update specific package
bun outdated               # Check for outdated packages
```

## Workflow File Structure

The CI workflow (`.github/workflows/ci.yml`) runs six jobs: `rust`, `python`, `frontend`, `e2e` (full-stack end-to-end with real services), `infra`, and `security`.

All source files, configs, and tests ship in the repo. CI jobs check out the repo and run directly — no scaffolding step required.

The `rust` job also runs `cargo sqlx prepare --check` to verify offline metadata is up to date.

The `e2e` job starts a full backend (Rust with PostgreSQL + Redis) and frontend, then runs Playwright-based e2e tests in \`frontend/tests/e2e/playwright/\`. It uses a dedicated `JWT_SECRET` and runs with `AUTH_MODE=cookie` for full auth flow testing.

```yaml
jobs:
  rust:    # fmt + clippy + sqlx check + cargo test
  python:  # ruff + pytest
  frontend: # lint + typecheck + vitest + build
  e2e:     # full-stack e2e with backend + frontend + real services
  infra:   # compose validation, Docker builds, Docker config checks
  security: # detect-secrets + gitleaks
```

## E2E Test Details

Tests use Playwright with Chromium (\`frontend/playwright.config.ts\`). Three spec files:

| Spec | Coverage |
|------|----------|
| \`auth.spec.ts\` | Login, register, redirect to dashboard, session persistence |
| \`dashboard.spec.ts\` | Health cards render, status dots, WebSocket fallback polling |
| \`notes.spec.ts\` | Create note → appears in list → delete → disappears |

CI installs Playwright via \`bunx playwright install --with-deps chromium\`. Screenshots and traces upload on failure (\`playwright-1\` artifact). Failed tests retry once.

The e2e job runs with a Redis service container, exercising the WebSocket live event code path end-to-end.

### E2E Auth Modes

CI runs e2e in two configurations:
- **Cookie auth** (default): Full Playwright suite — login form, session cookies, CSRF-protected CRUD
- **Bearer auth**: Limited subset — API-level auth flows for programmatic/SPA clients

Both modes share \`playwright.config.ts\` and spec files. \`AUTH_MODE\` env var selects routing behavior during test setup.

## Troubleshooting

### Common Issues

1. **Python 3.14 not found**
   - Use `actions/setup-python@v6` with `python-version: "3.14"` in the workflow step

2. **Socket permission denied**
   - Check that `CI=true` is set
   - Verify socket path in `.env`

3. **Benchmark failures**
   - Check if services are running before benchmarks
   - Use `make status` to debug

4. **Docker build failures**
   - Ensure Dockerfiles exist in `compose/`
   - Check build contexts are correct (should be `..` for repo root)

## Related Docs

- [Previous: ARCHITECTURE.md](./ARCHITECTURE.md) - System architecture
- [Next: MONITORING.md](./MONITORING.md) - Monitoring setup
- [All Docs](./INDEX.md) - Full documentation index
