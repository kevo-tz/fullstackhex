# Service Documentation

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Rust Backend (Workspace)](#rust-backend-workspace)
3. [py-api (Python Service)](#py-api-python-service)
4. [Frontend Service](#frontend-service)
5. [Service Communication](#service-communication)
6. [Health Checks](#health-checks)
7. [Related Docs](#related-docs)

## Architecture Overview

- **Rust Backend**: Main API server
- **py-api**: Independent process alongside Rust, internal-only communication via Unix socket
- **Frontend**: Communicates exclusively with Rust API (port 8001)

---

## Rust Backend (Workspace)

### Workspace Layout

```
backend/
├── Cargo.toml              # Workspace root
├── api/               # HTTP API layer (Axum routes)
├── auth/              # JWT + OAuth + CSRF authentication
├── cache/             # Redis caching, rate limiting, sessions
├── db/                # Database layer (sqlx)
├── domain/            # Business logic and shared types
├── py-sidecar/        # Unix socket client for Python IPC
└── storage/           # S3-compatible object storage
└── target/
```

### Crate: py-sidecar

Manages HTTP communication to py-api via Unix domain socket:

```rust
// py-sidecar/src/lib.rs
pub struct PythonSidecar {
    socket_path: PathBuf,
    timeout: Duration,
    max_retries: u32,
}

impl PythonSidecar {
    pub fn new(socket_path: impl Into<PathBuf>, timeout: Duration, max_retries: u32) -> Self { ... }

    /// Create from environment variables.
    pub fn from_env() -> Self { ... }

    /// Returns true if the socket file exists on disk.
    pub fn is_available(&self) -> bool { ... }

    /// HTTP GET over Unix socket with retry/backoff and timeout.
    pub async fn get(&self, path: &str) -> Result<serde_json::Value, SidecarError> { ... }

    /// Convenience: GET /health from the sidecar.
    pub async fn health(&self) -> Result<serde_json::Value, SidecarError> { ... }
}
```

### API Endpoints

#### Health Check
```
GET /health
```

Response (aggregated — all sub-services checked in a single call):
```json
{
  "rust":    { "status": "ok", "service": "api", "version": "0.13.0" },
  "db":      { "status": "ok" },
  "redis":   { "status": "ok" },
  "storage": { "status": "ok", "bucket": "fullstackhex" },
  "python":  { "status": "unavailable", "error": "socket not found", "fix": "Start the Python sidecar..." },
  "auth":    { "status": "ok" }
}
```

Individual sub-endpoints (`/health/db`, `/health/redis`, `/health/storage`, `/health/python`, `/health/auth`) return the same per-service JSON as the keys above.

#### Metrics (Prometheus)
```
GET /metrics
```

Returns Prometheus text format with:
- `http_requests_total` — Counter, labels: `method`, `route`, `status`
- `http_request_duration_seconds` — Histogram, labels: `method`, `route`
- `db_pool_connections` — Gauge, labels: `state` (`idle` / `used`)

```
GET /metrics/python
```

Proxies py-api metrics. Returns `503` if Python is unreachable.

```
GET /metrics/python  → Proxies py-api /metrics endpoint via Unix socket
```

### Running Rust with Sidecar

```bash
cd backend

# Development: starts Axum on port 8001
cargo run -p api

# The py-sidecar crate (py-sidecar/) contains the
# Unix socket client; wire it into main.rs to connect
# to the running py-api.
```

---

## py-api (Python Service)

### Running py-api

Python runs as an independent process alongside Rust, communicating over a Unix socket.
Start it via make targets (`make dev` or `make watch`) or manually:

```bash
cd py-api && uv run uvicorn app.main:app --uds /tmp/fullstackhex-python.sock
```

### FastAPI with Unix Socket

Requires `uv` (installed in Step 1 of SETUP.md).

```python
# app/main.py
from fastapi import FastAPI

app = FastAPI()


@app.get("/health")
def health() -> dict[str, str]:
    return {"status": "ok", "service": "py-api", "version": "0.13.0"}
```

> **Note:** Add new routes here as Python business logic grows. py-api runs
> independently — run it directly for development or debugging:
> `uv run uvicorn app.main:app --uds /tmp/fullstackhex-python.sock`

### Key Difference from Standalone

- **No direct external access**: Only accessible via Rust proxy through Unix socket
- **Independent process**: Python runs as a separate process alongside Rust (started by `make dev` or `make watch`)
- **Internal networking**: Communicates via `/tmp/fullstackhex-python.sock`

---

## Frontend Service (Astro + Bun)

### Implementation Goal

The frontend is a template-ready Astro application managed by **Bun**. It serves a single `index.astro` page, uses **Tailwind** for styling, and keeps backend integration behind Astro-owned server routes when possible.

### Recommended Structure

```text
frontend/
├── astro.config.mjs
├── package.json
├── tsconfig.json
├── public/
└── src/
        ├── components/
        ├── layouts/
        └── pages/
                ├── index.astro
                └── api/
                        └── health.ts
```

> **Note:** Tailwind v4 uses `@tailwindcss/vite` as a Vite plugin — no `tailwind.config.mjs` is needed.

### Scaffold Frontend

```bash
# Create Astro app with Bun
bun create astro@latest frontend -- --template minimal --no-install --no-git --yes

cd frontend

# Install Tailwind v4 and the Node SSR adapter
bun add @tailwindcss/vite tailwindcss @astrojs/node

# Install all dependencies
bun install
```

`astro.config.mjs` must enable SSR and add the Tailwind vite plugin:

```javascript
// astro.config.mjs
import { defineConfig } from 'astro/config';
import node from '@astrojs/node';
import tailwindcss from '@tailwindcss/vite';

export default defineConfig({
  output: 'server',
  adapter: node({ mode: 'standalone' }),
  vite: {
    plugins: [tailwindcss()]
  }
});
```

### API Communication (Rust Only)

Frontend browser code should call Astro-owned routes first. Astro server routes then call Rust when backend data is needed:

```typescript
// src/pages/api/health.ts
export async function GET() {
    const response = await fetch(`${import.meta.env.VITE_RUST_BACKEND_URL}/health`);
    const body = await response.json();

    return new Response(JSON.stringify(body), {
        headers: { 'Content-Type': 'application/json' },
    });
}
```

Page code then calls the frontend-owned route:

```typescript
const response = await fetch('/api/health');

// Astro server route calls Rust internally
// Frontend still doesn't know/care about Python
```

### Environment Configuration

```env
# Root .env (used by both backend and frontend)
ASTRO_PORT=4321
PUBLIC_API_URL=http://localhost:8001
VITE_RUST_BACKEND_URL=http://localhost:8001
# No VITE_PYTHON_SERVICE_URL - frontend doesn't talk to Python directly
```

### Single-Page Template Scope

The first implementation should stay intentionally narrow:

- **Page:** `src/pages/index.astro`
- **Purpose:** Explain the Rust/Bun/uv stack and show service entry points
- **UI blocks:** Stack summary, service cards, backend health status, quick-start commands
- **Client JavaScript:** Minimal; prefer Astro rendering first

### Running Frontend

```bash
cd frontend

# Install dependencies
bun install

# Development server with HMR
bun run dev --port 4321

# Production build
bun run build

# Preview production build
bun run preview
```

---

## Service Communication

### Rust → Python (Unix Socket)

```rust
// From Rust crate: py-sidecar
use std::time::Duration;

async fn call_python() -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let sidecar = PythonSidecar::new(
        "/tmp/fullstackhex-python.sock",
        Duration::from_secs(5),
        3,
    );
    let result = sidecar.get("/health").await?;
    Ok(result)
}
```

### Frontend → Rust

```typescript
// Browser -> Astro server route
async function getHealth() {
    const response = await fetch('/api/health');
    return response.json();
}
```

---

## Health Checks

```bash
# Rust backend
curl http://localhost:8001/health

# py-api (via Rust)
curl http://localhost:8001/health/python

# Frontend
curl -I http://localhost:4321

# Infrastructure
docker compose ps
```

---

## Log Locations

| Service | Log destination | How to tail |
|---------|----------------|-------------|
| Rust backend | stdout (runs directly) | `make logs` |
| py-api | stdout (runs directly) | `make logs` |
| Frontend (Astro) | stdout (runs directly) | `make logs` |
| PostgreSQL | Docker container stdout | `make logs` |
| Redis | Docker container stdout | `make logs` |
| RustFS | Docker container stdout | `docker compose -f compose/dev.yml logs -f rustfs` |
| Prometheus | Docker container stdout | `docker compose -f compose/monitor.yml logs -f prometheus` |
| Grafana | Docker container stdout | `docker compose -f compose/monitor.yml logs -f grafana` |

Docker services also support: `docker compose -f compose/dev.yml logs -f <service>`

---

## Related Docs

- [Previous: ARCHITECTURE.md](./ARCHITECTURE.md) - System design overview
- [Next: INFRASTRUCTURE.md](./INFRASTRUCTURE.md) - Docker setup and config
- [All Docs](./INDEX.md) - Full documentation index
