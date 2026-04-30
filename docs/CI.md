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
- **Frontend**: `bun test`
- **Python**: `pytest`

### 4. Performance Gates (Optional)
- Runs `scripts/bench.sh` if `RUN_BENCHMARKS=true` is set
- Checks p50 < 5ms, p99 < 20ms for /health endpoint
- TTFB < 0.1s for frontend

### 5. Build Docker Images (main branch only)
- Builds production images using `compose/prod.yml`
- Pushes to container registry (if configured)

## Required Secrets

Configure these in GitHub repository settings (**Settings → Secrets and variables → Actions**):

| Secret | Description | Required For |
|--------|-------------|--------------|
| `POSTGRES_PASSWORD` | Database password | Tests |
| `REDIS_PASSWORD` | Redis password | Tests |
| `RUSTFS_ACCESS_KEY` | S3-compatible storage access key | Integration tests |
| `RUSTFS_SECRET_KEY` | S3-compatible storage secret key | Integration tests |
| `DOCKER_USERNAME` | Docker Hub username | Image publishing |
| `DOCKER_PASSWORD` | Docker Hub password/token | Image publishing |

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
cd python-sidecar
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
make test-rust
make test-python
make test-frontend

# Run with verbose output
cd backend && cargo test -- --nocapture
cd python-sidecar && uv run pytest -v
cd frontend && bun test --verbose
```

### Socket Path Issues in CI

The `install.sh` script automatically detects `CI=true` and uses a temp directory for the Unix socket:

```bash
# In CI, socket goes to $RUNNER_TEMP or $PWD/.tmp
export CI=true
./scripts/install.sh
```

To debug socket issues locally:

```bash
# Simulate CI environment
CI=true ./scripts/install.sh
./scripts/verify-health.sh
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
cd python-sidecar
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

The CI workflow (`.github/workflows/ci.yml`) typically includes:

```yaml
name: CI

on:
  push:
    branches: [main, develop]
  pull_request:
    branches: [main]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
      - name: Install Bun
        uses: oven-sh/setup-bun@v1
      - name: Run install script
        run: ./scripts/install.sh --skip-python
      - name: Run tests
        run: make test
```

## Troubleshooting

### Common Issues

1. **Python 3.14 not found**
   - Use `--skip-python` flag in CI
   - Or set up pyenv in workflow

2. **Socket permission denied**
   - Check that `CI=true` is set
   - Verify socket path in `.env`

3. **Benchmark failures**
   - Check if services are running before benchmarks
   - Use `./scripts/verify-health.sh` to debug

4. **Docker build failures**
   - Ensure Dockerfiles exist in `compose/`
   - Check build contexts are correct (should be `..` for repo root)

## Related Docs

- [Previous: ARCHITECTURE.md](./ARCHITECTURE.md) - System architecture
- [Next: MONITORING.md](./MONITORING.md) - Monitoring setup
- [All Docs](./INDEX.md) - Full documentation index
