# Performance Budget

Performance targets enforced via CI gates. Any missed target becomes a P1 item.

## Prerequisites

`scripts/bench.sh` requires `ab` (Apache Bench) for load testing:

```bash
# Install ab (Apache Bench)
# Linux (Debian/Ubuntu):
sudo apt-get install apache2-utils
# Linux (RHEL/CentOS):
sudo yum install httpd-tools
# macOS:
# ab is included with Apache (or brew install httpd)

# Verify installation
ab -V
```

## Targets

| Metric | Target | Measurement |
|--------|--------|-------------|
| `/health` p50 latency | < 5ms | `ab -n 1000 -c 100 http://localhost:8001/health` |
| `/health` p99 latency | < 20ms | same |
| Rust → Python sidecar roundtrip | < 2ms local | Rust calls Python over Unix socket |
| Postgres query (simple read) | < 10ms p99 | `sqlx --quiet` timings |
| Frontend TTFB (SSR) | < 100ms | `curl -w "%{time_starttransfer}"` |
| Memory per Rust worker | < 50MB RSS | `ps aux` after warmup |
| Hot path allocations | 0 allocs in p99 path | `cargo flamegraph` on /health |

## CI Enforcement

Targets are enforced as follows:

- **Load testing baseline** (`scripts/bench.sh`): runs on every tagged release
- **sqlx offline check**: `cargo sqlx prepare --check` in CI
- **p99 regression threshold**: build fails if p99 regresses > 20% vs previous baseline

## Adding New Targets

When adding performance targets:
1. Define metric, target, and measurement command in this file
2. Add to CI baseline script
3. Document in `scripts/bench.sh`

## Related Docs

- [Previous: INITIALIZATION.md](./INITIALIZATION.md) - Project initialization template
- [All Docs](./INDEX.md) - Full documentation index