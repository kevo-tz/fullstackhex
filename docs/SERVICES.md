# Service Documentation

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

## Frontend Service

### API Communication (Rust Only)

Frontend ONLY communicates with Rust backend:

```typescript
// Correct: Call Rust API
const response = await fetch('http://localhost:8001/api/data');

// Rust proxies to Python internally when needed
// Frontend doesn't know/care about Python
```

### Environment Configuration

```env
# .env (frontend)
VITE_RUST_BACKEND_URL=http://localhost:8001
# No VITE_PYTHON_SERVICE_URL - frontend doesn't talk to Python directly
```

### Running Frontend

```bash
cd frontend

# Install dependencies
bun install

# Development server with HMR
bun run dev  # Port 4321

# Production build
bun run build
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
// From TypeScript
async function getData() {
    const response = await fetch('http://localhost:8001/api/data');
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

## Next Steps

- See [INFRASTRUCTURE.md](./INFRASTRUCTURE.md) for Docker infrastructure (single source of truth)
- See [SETUP.md](./SETUP.md) for installation and troubleshooting
- See [ARCHITECTURE.md](./ARCHITECTURE.md) for system design details
- See [INITIALIZATION.md](./INITIALIZATION.md) for template-ready setup
