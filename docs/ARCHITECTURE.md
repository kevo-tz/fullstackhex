# System Architecture Overview

## Table of Contents

1. [Core Architecture](#core-architecture-rust-centric-with-python-sidecar)
2. [Key Architectural Decisions](#key-architectural-decisions)
3. [Data Flow](#data-flow)
4. [Technology Stack (Latest Versions)](#technology-stack-latest-versions)
5. [Workspace Structure](#workspace-structure)
6. [IPC: Unix Domain Socket](#ipc-unix-domain-socket)
7. [Port Mappings](#port-mappings)
8. [Related Docs](#related-docs)

## Core Architecture: Rust-Centric with Python Sidecar

```
┌────────────────────────────────────────────────────────┐
│          Frontend (Astro + Bun)                       │
│              Port 4321                                 │
└─────────────────┬──────────────────────────────────────┘
                  │ HTTP API only
                  ▼
┌────────────────────────────────────────────────────────┐
│         Rust Backend (Axum + Tokio)                   │
│              Port 8001 (only external API)            │
│                                                        │
│  Workspace Crates:                                    │
│  ├── api/          HTTP routes, middleware            │
│  ├── core/         Business logic                    │
│  ├── db/           sqlx + PostgreSQL                 │
│  └── python-sidecar/ Sidecar manager                │
│                        │                              │
│                        │ Unix domain socket            │
│                        ▼                              │
│  ┌─────────────────────────────────────────┐         │
│  │    Python Service (FastAPI)             │         │
│  │    /tmp/python-sidecar.sock (internal) │         │
│  └─────────────────────────────────────────┘         │
└──────────────┬───────────────────────────────────────┘
               │
    ┌──────────┼──────────┐
    ▼          ▼          ▼
┌────────┐ ┌────────┐ ┌─────────┐
│ Postgres│ │ Redis  │ │ RustFS  │
│  5432   │ │ 6379   │ │  9000   │
└────────┘ └────────┘ └─────────┘
```

### Production Additions (not active in dev)

```
┌─────────────────────────────────────────┐
│ Nginx (Reverse Proxy)                  │
│   :80 HTTP  /  :443 HTTPS              │
│   Sits in front of Frontend + Backend  │
└─────────────────────────────────────────┘

┌─────────────────────────────────────────┐
│ Monitoring Stack                       │
│  ├── Prometheus :9090  (scrapes all)   │
│  └── Grafana    :3000  (dashboards)    │
└─────────────────────────────────────────┘
```

## Key Architectural Decisions

| Decision | Rationale |
|----------|-----------|
| Python as Rust sidecar | Single entry point, Rust controls lifecycle |
| Frontend → Rust only | Simplified networking, Rust proxies to Python |
| Rust workspace | Modular crates, clear boundaries |
| Unix domain socket | Fast IPC on Linux/macOS, no TCP overhead |
| Latest versions | Predictable initialization via scripts |

## Data Flow

1. **Browser** → Requests Astro frontend on `localhost:4321`
2. **Astro Server Route** → Calls Rust backend on `localhost:8001` when backend data needed
3. **Rust API** → Processes request, may call internal crates
4. **Python Sidecar** → Rust communicates via Unix domain socket when Python logic needed
5. **Data Layer** → Postgres (sqlx) + Redis + RustFS
6. **Production only** → Nginx terminates TLS and proxies external traffic; Prometheus + Grafana monitor the entire stack

## Technology Stack (Latest Versions)

| Component | Technology | Version Check Command |
|-----------|-------------|----------------------|
| Rust | Edition 2024 | `rustc --version` |
| Workspace | Cargo workspace | Auto |
| Web Framework | Axum 0.8+ | Check crates.io |
| Async Runtime | Tokio 1.x | Check crates.io |
| Python | 3.14+ | `python3 --version` |
| Package Manager | uv (latest) | `uv --version` |
| Frontend | Astro 6.x + Bun | `bun --version` |
| Database | PostgreSQL 18 | `docker compose ps` |
| Cache | Redis 8 | `docker compose ps` |
| Object Storage | RustFS (S3-compatible) | `docker compose ps` |
| Monitoring | Prometheus 3.x + Grafana | Production only, see .env.example |
| Reverse Proxy | Nginx (production) | Production only, see .env.example |
| IPC | Unix domain socket | `/tmp/python-sidecar.sock` |

## Workspace Structure

```
frontend/
├── astro.config.mjs         # Astro SSR config (output: server, @astrojs/node adapter)
├── package.json             # Bun-managed scripts and dependencies
├── tsconfig.json            # TypeScript config (extends astro/tsconfigs/strict)
├── src/
│   └── pages/
│       ├── index.astro      # Template landing page
│       └── api/
│           └── health.ts    # Astro server route proxying Rust health
└── tests/                   # Bun test suites (unit, integration, smoke)

backend/
├── Cargo.toml              # Workspace root
├── crates/
│   ├── api/               # HTTP API layer (Axum routes)
│   ├── core/              # Business logic
│   ├── db/                # Database layer (sqlx)
│   └── python-sidecar/    # Sidecar process manager
└── target/
```

## IPC: Unix Domain Socket

Python sidecar binds to `/tmp/python-sidecar.sock`. Rust communicates through this socket for:
- Low latency (no TCP overhead)
- Security (only local processes can connect)
- Simple integration with FastAPI/Uvicorn

### How Rust Spawns Python Subprocess

```rust
// backend/crates/python-sidecar/src/lib.rs
use std::process::{Command, Child};
use std::path::PathBuf;

pub struct PythonSidecar {
    process: Option<Child>,
    socket_path: PathBuf,
}

impl PythonSidecar {
    pub fn start(socket_path: PathBuf) -> Result<Self, std::io::Error> {
        // Ensure socket directory exists
        if let Some(parent) = socket_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Remove stale socket file if exists
        if socket_path.exists() {
            std::fs::remove_file(&socket_path)?;
        }

        // Spawn Python FastAPI with Uvicorn
        let child = Command::new("uvicorn")
            .args([
                "app.main:app",
                "--uds", socket_path.to_str().unwrap(),
                "--workers", "1",
            ])
            .current_dir("python-sidecar")
            .spawn()?;

        Ok(Self {
            process: Some(child),
            socket_path,
        })
    }
}

impl Drop for PythonSidecar {
    fn drop(&mut self) {
        if let Some(mut child) = self.process.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        // Clean up socket file
        if self.socket_path.exists() {
            let _ = std::fs::remove_file(&self.socket_path);
        }
    }
}
```

### Sending HTTP Requests Over Unix Socket

```rust
// Using hyper with Unix socket support
use hyper::Client;
use hyper::client::connect::dns::local::Local;
use hyper_unix_connector::UnixClient;

async fn call_python_sidecar(socket_path: &str, path: &str) -> Result<String, Box<dyn std::error::Error>> {
    // Create Unix socket connector
    let connector = UnixClient::new(socket_path)?;
    let client = Client::builder().build::<_, hyper::Body>(connector);

    // Build request to Python sidecar
    let url = format!("http://localhost{}", path);  // localhost is ignored for Unix sockets
    let req = hyper::Request::builder()
        .method("GET")
        .uri(url)
        .body(hyper::Body::empty())?;

    // Send request
    let resp = client.request(req).await?;
    let body = hyper::body::to_bytes(resp.into_body()).await?;

    Ok(String::from_utf8(body.to_vec())?)
}
```

### Error Handling for Socket Failures

```rust
use std::error::Error;
use tokio::time::{timeout, Duration};

#[derive(Debug)]
pub enum SidecarError {
    SocketNotFound(PathBuf),
    ConnectionFailed(String),
    Timeout(String),
    InvalidResponse(String),
}

async fn call_sidecar_with_retry(
    socket_path: &PathBuf,
    path: &str,
    max_retries: u32,
) -> Result<String, SidecarError> {
    // Check if socket exists before trying
    if !socket_path.exists() {
        return Err(SidecarError::SocketNotFound(socket_path.clone()));
    }

    let mut last_error = None;

    for attempt in 1..=max_retries {
        match timeout(
            Duration::from_secs(5),
            call_python_sidecar(socket_path.to_str().unwrap(), path),
        )
        .await
        {
            Ok(Ok(response)) => return Ok(response),
            Ok(Err(e)) => {
                last_error = Some(SidecarError::ConnectionFailed(e.to_string()));
                tokio::time::sleep(Duration::from_millis(100 * attempt as u64)).await;
            }
            Err(_) => {
                last_error = Some(SidecarError::Timeout(path.to_string()));
                tokio::time::sleep(Duration::from_millis(100 * attempt as u64)).await;
            }
        }
    }

    Err(last_error.unwrap_or(SidecarError::Timeout(path.to_string())))
}
```

### Configuration via Environment

```rust
// Read socket path from environment
fn get_socket_path() -> PathBuf {
    std::env::var("PYTHON_SIDECAR_SOCKET")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp/python-sidecar.sock"))
}
```

## Port Mappings

| Service | Port | Purpose |
|---------|------|---------|
| Frontend | 4321 | Development server |
| Rust Backend | 8001 | Only external API |
| Python Sidecar | Internal | Unix socket only (/tmp/python-sidecar.sock) |
| PostgreSQL | 5432 | Database |
| Redis | 6379 | Cache |
| RustFS | 9000 | S3-compatible storage |
| RustFS | 9001 | Console for storage |
| Prometheus | 9090 | Metrics collection (production) |
| Grafana | 3000 | Monitoring dashboards (production) |
| Nginx | 80/443 | Reverse proxy (production) |


## Related Docs

- [Previous: SETUP.md](./SETUP.md) - One-command init and tool install
- [Next: SERVICES.md](./SERVICES.md) - Service details and communication
- [All Docs](./INDEX.md) - Full documentation index
