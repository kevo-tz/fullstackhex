# Logging Conventions

FullStackHex uses structured JSON logging across all three languages with a consistent field schema. Every log line is a standalone JSON object on stderr (Rust, Python) or stdout (TypeScript).

## Field Schema

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `timestamp` | string (ISO 8601) | yes | When the event occurred |
| `level` | string | yes | `info`, `warn`, `error`, `debug` |
| `target` | string | yes | Module or component name |
| `message` | string | yes | Human-readable description |
| `trace_id` | string | no | UUIDv4 for cross-language correlation |
| `duration_ms` | number | no | Operation duration in milliseconds |
| `error` | string | no | Error description (present on failures) |

## Trace ID Ownership

- **Rust is the originator** for all socket requests: generates UUIDv4 via the `uuid` crate, sends as `x-trace-id` HTTP header over the Unix socket
- **Python extracts and logs** `x-trace-id` from incoming requests. Python never generates its own trace_id for incoming requests
- **TypeScript originates** trace_ids for frontend-initiated health check poll cycles via `crypto.randomUUID()`

## Log Levels

| Level | Usage |
|-------|-------|
| `info` | Normal operations: request received, health check, service started |
| `warn` | Recoverable errors: socket timeout that will be retried, degraded service |
| `error` | Unrecoverable errors: socket not found, invalid response, database connection failure |
| `debug` | Verbose diagnostics (not used in production) |

## Cross-Language Correlation

To trace a request across all three languages:

```bash
grep <trace_id> rust.log python.log frontend.log
```

Each log line with the same `trace_id` represents one step in the request's journey through the system.

## Implementation

- **Rust:** `tracing-subscriber` with JSON layer, writing to stderr. Configured in `backend/crates/api/src/main.rs`.
- **Python:** Custom `JsonFormatter` on `logging.StreamHandler(sys.stderr)`. Configured in `python-sidecar/app/main.py`.
- **TypeScript:** Lightweight `jsonLog()` wrapper over `console.log` on Astro server stdout. Used in `frontend/src/lib/health.ts`.
