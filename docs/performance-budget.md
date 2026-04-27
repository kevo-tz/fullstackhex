# Performance Budget

Performance targets enforced via CI gates. Any missed target becomes a P1 item.

## Targets

| Metric | Target | Measurement |
|--------|--------|-------------|
| `/api/health` p50 latency | < 5ms | `bombardier -c 100 -d 30s http://localhost:8001/health` |
| `/api/health` p99 latency | < 20ms | same |
| Rust → Python sidecar roundtrip | < 2ms local | Rust calls Python over Unix socket |
| Postgres query (simple read) | < 10ms p99 | `sqlx --quiet` timings |
| Frontend TTFB (SSR) | < 100ms | `curl -w "%{time_starttransfer}"` |
| Memory per Rust worker | < 50MB RSS | `ps aux` after warmup |
| Zero-allocation hot paths | true | `cargo flamegraph` on p99 path |

## CI Enforcement

Targets are enforced as follows:

- **Load testing baseline** (`scripts/bench.sh`): runs on every tagged release
- **sqlx migration gate**: `cargo sqlx migrate verify` in CI
- **p99 regression threshold**: build fails if p99 regresses > 20% vs previous baseline

## Adding New Targets

When adding performance targets:
1. Define metric, target, and measurement command in this file
2. Add to CI baseline script
3. Document in `scripts/bench.sh`