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
rust-backend/
├── Cargo.toml              # Workspace root
├── crates/
│   ├── api/               # HTTP API layer (Axum routes)
│   ├── core/              # Business logic
│   ├── db/                # Database layer (sqlx)
│   └── python-sidecar/    # Sidecar process manager
└── target/
```

### Crate: python-sidecar

Manages Python process via Unix domain socket:

```rust
// crates/python-sidecar/src/lib.rs
use tokio::net::UnixStream;
use std::path::PathBuf;

pub struct PythonSidecar {
    socket_path: PathBuf,  // /tmp/python-sidecar.sock
}

impl PythonSidecar {
    pub async fn call(&self, request: &str) -> Result<String> {
        let mut stream = UnixStream::connect(&self.socket_path).await?;
        stream.write_all(request.as_bytes()).await?;
        
        let mut response = String::new();
        stream.read_to_string(&mut response).await?;
        Ok(response)
    }
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

#### Proxy to Python (internal)
```
POST /api/python/{path}  → Forwards to Python sidecar via Unix socket
```

### Running Rust with Sidecar

```bash
cd rust-backend

# Development
cargo run --workspace

# Rust automatically:
# 1. Starts the Axum server on 8001
# 2. Spawns Python sidecar process
# 3. Connects via Unix domain socket
# 4. Manages sidecar lifecycle (restart on crash)
```

---

## Python Service (Sidecar Mode)

### Running as Sidecar

Python runs as a subprocess of Rust, not standalone:

```bash
# This is managed by Rust, not run manually
uv run uvicorn src.main:app --uds /tmp/python-sidecar.sock
```

### FastAPI with Unix Socket

Requires `uv` (installed in Step 1 of SETUP.md).

```python
# src/main.py
from fastapi import FastAPI
import uvicorn

app = FastAPI()

@app.get("/health")
async def health():
    return {"status": "ok"}

@app.post("/process")
async def process(data: dict):
    # Python business logic here
    return {"result": "processed"}

if __name__ == "__main__":
    # Run on Unix socket, not TCP port
    uvicorn.run(
        app,
        uds="/tmp/python-sidecar.sock"  # Internal only
    )
```

### Key Difference from Standalone

- **No direct external access**: Only accessible via Rust proxy through Unix socket
- **Managed lifecycle**: Rust restarts Python on crash
- **Internal networking**: Communicates via `/tmp/python-sidecar.sock`

---

## Frontend Service (Astro + Bun)

### Implementation Goal

The frontend is a template-ready Astro application managed by **Bun**. It serves a single `index.astro` page, uses **Tailwind** for styling, and keeps backend integration behind Astro-owned server routes when possible.

### Recommended Structure

```text
frontend/
├── astro.config.mjs
├── package.json
├── tailwind.config.mjs
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

### Scaffold Frontend

```bash
# Create Astro app with Bun
bun create astro@latest frontend

# Enter project
cd frontend

# Add Tailwind integration
bunx astro add tailwind

# Install dependencies
bun install
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
use tokio::net::UnixStream;
use serde_json::json;

pub async fn call_python() -> Result<String> {
    let mut stream = UnixStream::connect("/tmp/python-sidecar.sock").await?;
    
    let request = json!({
        "method": "POST",
        "path": "/process",
        "body": {"data": "example"}
    }).to_string();
    
    stream.write_all(request.as_bytes()).await?;
    
    let mut response = String::new();
    stream.read_to_string(&mut response).await?;
    Ok(response)
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
