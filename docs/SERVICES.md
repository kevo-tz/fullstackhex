# Service Documentation

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Rust Backend (Workspace)](#rust-backend-workspace)
3. [Python Service (Sidecar Mode)](#python-service-sidecar-mode)
4. [Frontend Service](#frontend-service)
5. [Service Communication](#service-communication)
6. [Health Checks](#health-checks)
7. [Related Docs](#related-docs)

## Architecture Overview

- **Rust Backend**: Main API server, manages Python sidecar
- **Python Sidecar**: Managed subprocess of Rust, internal-only communication via Unix socket
- **Frontend**: Communicates exclusively with Rust API (port 8001)

---

## Rust Backend (Workspace)

### Workspace Layout

```
backend/
├── Cargo.toml              # Workspace root
├── crates/
│   ├── api/               # HTTP API layer (Axum routes)
│   ├── core/              # Business logic
│   ├── db/                # Database layer (sqlx)
│   └── python-sidecar/    # Sidecar process manager
└── target/
```

### Crate: python-sidecar

Manages HTTP communication to the Python sidecar via Unix domain socket:

```rust
// crates/python-sidecar/src/lib.rs
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

Response:
```json
{
  "status": "ok"
}
```

> **Note:** The Python sidecar `/health` response includes an additional field: `{"status": "ok", "service": "python-sidecar"}`.
```
ANY /api/python/{path}  → Forwards request method/path/body to Python sidecar via Unix socket
```

### Running Rust with Sidecar

```bash
cd backend

# Development: starts Axum on port 8001
cargo run -p api

# The python-sidecar crate (crates/python-sidecar/) contains the
# Unix socket client; wire it into main.rs to spawn and connect
# to the Python sidecar process.
```

---

## Python Service (Sidecar Mode)

### Running as Sidecar

Python runs as a subprocess of Rust, not standalone:

```bash
# This is managed by Rust, not run manually
uv run uvicorn app.main:app --uds /tmp/fullstackhex-python.sock
```

### FastAPI with Unix Socket

Requires `uv` (installed in Step 1 of SETUP.md).

```python
# app/main.py
from fastapi import FastAPI

app = FastAPI()


@app.get("/health")
def health() -> dict[str, str]:
    return {"status": "ok", "service": "python-sidecar"}
```

> **Note:** Add new routes here as Python business logic grows. The sidecar is
> started by Rust — run it directly only for debugging:
> `uv run uvicorn app.main:app --uds /tmp/fullstackhex-python.sock`

### Key Difference from Standalone

- **No direct external access**: Only accessible via Rust proxy through Unix socket
- **Managed lifecycle**: Rust restarts Python on crash
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
# .env (frontend)
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
// From Rust crate: python-sidecar
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

# Python sidecar (via Rust)
curl http://localhost:8001/api/python/health

# Frontend
curl -I http://localhost:4321

# Infrastructure
docker compose ps
```

---

## Related Docs

- [Previous: ARCHITECTURE.md](./ARCHITECTURE.md) - System design overview
- [Next: INFRASTRUCTURE.md](./INFRASTRUCTURE.md) - Docker setup and config
- [All Docs](./INDEX.md) - Full documentation index
