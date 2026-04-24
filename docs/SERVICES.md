# Service Documentation

## Overview

The Bare Metal Demo consists of three main services, each with specific responsibilities and technology choices. This document provides detailed information about each service, including how to run them, their endpoints, and how they interact.

---

## Rust Backend Service

### Framework: Axum

Axum is a modular and composable web framework built on top of Tokio. It provides excellent performance and type safety.

**Key Benefits**:
- Compile-time safety for database queries (via sqlx)
- Async/await native support
- Excellent middleware ecosystem
- Type-safe routing

### Service Details

**Location**: `/home/cevor/github/bare_metal_demo/rust-backend`
**Language**: Rust (Edition 2024)
**Runtime**: Tokio (async)
**Default Port**: 3000

### Dependencies

```toml
axum = "0.8"              # Web framework
tokio = "1"               # Async runtime
sqlx = "0.8"              # Database driver with compile-time checking
serde = "1.0"             # Serialization
tower-http = "0.5"        # Middleware (CORS, tracing)
dotenv = "0.15"           # Environment configuration
chrono = "0.4"            # Date/time handling
uuid = "1.0"              # Unique identifiers
log/env_logger            # Logging
```

### Running the Service

#### Prerequisites
- Rust 1.95+ installed
- PostgreSQL running on port 5432 (or configure via env var)

#### Development

```bash
cd rust-backend

# Build the project
cargo build

# Run with debug logging
RUST_LOG=info cargo run

# Run with hot reload (requires cargo-watch)
cargo watch -q -c -w src -x run
```

#### Release Build

```bash
cargo build --release
./target/release/bare-metal-rust
```

### Database

**Database Driver**: sqlx (async PostgreSQL driver)

**Connection Details**:
```
Host: localhost
Port: 5432
User: rust_user
Password: rust_pass
Database: rust_service
```

**Configuration**:
```rust
// From main.rs
let database_url = std::env::var("RUST_SERVICE_DB_URL")
    .unwrap_or_else(|_| "postgres://localhost/bare_metal".to_string());

let pool = PgPoolOptions::new()
    .max_connections(5)
    .connect(&database_url)
    .await?;
```

**Connection Pool**: 5 maximum connections (configurable)

### Endpoints

#### Health Check
```
GET /health
```

**Response**:
```json
{
  "status": "ok"
}
```

**Status Code**: 200

**Use Case**: Service health verification, monitoring

### Architecture

```
main.rs
  ├── AppState (shared application state)
  │   └── db_pool (Arc<sqlx::PgPool>)
  ├── Router (route handlers)
  │   └── /health -> health_check()
  └── TcpListener (0.0.0.0:3000)
```

### Middleware Stack

```rust
Router::new()
    .route("/health", get(health_check))
    .layer(CorsLayer::permissive())  // CORS enabled
    .layer(TraceLayer)                // Request tracing (optional)
    .with_state(state)                // Application state
```

**CORS Configuration**: Currently permissive (accepts all origins)
- Production: Configure specific allowed origins
- Headers: All allowed
- Methods: GET, POST, PUT, DELETE, PATCH allowed

### Logging

**Configure via environment variable**:
```bash
export RUST_LOG=debug
cargo run
```

**Log Levels**:
- `error`: Only errors
- `warn`: Errors and warnings
- `info`: General information (default)
- `debug`: Detailed debug information
- `trace`: Very detailed trace information

**Output**: Logs to stdout (configured via env_logger)

### Database Migrations

Place migration files in `migrations/` directory if using sqlx migrations:

```bash
# Install sqlx CLI
cargo install sqlx-cli

# Create a new migration
sqlx migrate add -r initialize_database

# Run migrations
sqlx migrate run

# Revert migrations
sqlx migrate revert
```

### Building and Deployment

```bash
# Development
cargo build
cargo run

# Release with optimizations
cargo build --release
./target/release/bare-metal-rust

# Docker
docker build -t bare-metal-rust .
docker run -p 3000:3000 \
  -e RUST_SERVICE_DB_URL=postgres://... \
  bare-metal-rust
```

---

## Python Services

### Framework: FastAPI

FastAPI is a modern, fast (high-performance) web framework for building APIs with Python 3.12+. It's based on standard Python type hints.

**Key Benefits**:
- Automatic interactive API documentation (Swagger UI)
- Type hints for validation
- Fast execution (similar to Node.js/Go)
- Excellent async/await support

### Service Details

**Location**: `/home/cevor/github/bare_metal_demo/python-services`
**Language**: Python 3.14
**Framework**: FastAPI
**ASGI Server**: uvicorn
**Default Port**: 8001
**Dependency Manager**: UV (recommended) or pip

### Dependencies

```toml
fastapi = ">=0.104.0"           # Web framework
uvicorn = ">=0.24.0"            # ASGI server
psycopg = ">=3.1.0"             # Async PostgreSQL driver
pydantic = ">=2.0.0"            # Data validation
python-dotenv = ">=1.0.0"       # Environment configuration
```

### Running the Service

#### Prerequisites
- Python 3.14 installed
- UV or pip
- PostgreSQL running on port 5433 (or configure via env var)

#### Using UV (Recommended)

```bash
cd python-services

# Install dependencies
uv sync

# Run development server with hot reload
uv run python -m uvicorn src.main:app --host 0.0.0.0 --port 8001 --reload

# Run production server
uv run python -m uvicorn src.main:app --host 0.0.0.0 --port 8001 --workers 4
```

#### Using pip

```bash
cd python-services

# Create virtual environment
python -m venv venv
source venv/bin/activate  # On Windows: venv\Scripts\activate

# Install dependencies
pip install -e .

# Run development server
python -m uvicorn src.main:app --host 0.0.0.0 --port 8001 --reload

# Run production server
python -m uvicorn src.main:app --host 0.0.0.0 --port 8001 --workers 4
```

### Database

**Database Driver**: psycopg (async PostgreSQL driver)

**Connection Details**:
```
Host: localhost
Port: 5433
User: python_user
Password: python_pass
Database: python_service
```

**Configuration**:
```python
from src.main import DB_URL

DB_URL = os.getenv(
    "PYTHON_SERVICE_DB_URL",
    "postgresql://python_user:python_pass@localhost:5433/python_service"
)
```

### Endpoints

#### Health Check
```
GET /health
```

**Response**:
```json
{
  "status": "ok"
}
```

**Status Code**: 200

**Use Case**: Service health verification, monitoring

### API Documentation

Once the service is running, access interactive documentation:

- **Swagger UI**: http://localhost:8001/docs
- **ReDoc**: http://localhost:8001/redoc
- **OpenAPI JSON**: http://localhost:8001/openapi.json

### Architecture

```
src/main.py
  ├── FastAPI app initialization
  ├── Lifespan context manager
  │   ├── startup: Initialize services
  │   └── shutdown: Clean up resources
  ├── Router
  │   └── GET /health -> health_check()
  └── uvicorn.run() configuration
```

### Application Structure

```python
from fastapi import FastAPI

@app.get("/health", response_model=HealthResponse)
async def health_check():
    """Health check endpoint."""
    return HealthResponse(status="ok")

if __name__ == "__main__":
    uvicorn.run(
        app,
        host="0.0.0.0",
        port=8001
    )
```

### Logging

**Configure logging**:
```python
import logging

logger = logging.getLogger(__name__)
logger.info("Service started")
```

**Uvicorn log levels**:
```bash
# Critical
python -m uvicorn src.main:app --log-level critical

# Error (default for production)
python -m uvicorn src.main:app --log-level error

# Warning
python -m uvicorn src.main:app --log-level warning

# Info
python -m uvicorn src.main:app --log-level info

# Debug
python -m uvicorn src.main:app --log-level debug

# Trace
python -m uvicorn src.main:app --log-level trace
```

### Database Migrations

Using Alembic for migrations (if implemented):

```bash
# Install Alembic
uv add alembic

# Initialize migrations
alembic init migrations

# Create a migration
alembic revision --autogenerate -m "Initial schema"

# Apply migrations
alembic upgrade head

# Rollback migrations
alembic downgrade -1
```

### Development Tools

```bash
# Code formatting
uv run black src/

# Linting
uv run ruff check src/

# Testing
uv run pytest tests/

# Type checking
uv run mypy src/
```

### Building and Deployment

```bash
# Development
uv run python -m uvicorn src.main:app --reload

# Production with Gunicorn
uv add gunicorn
uv run gunicorn src.main:app --workers 4 --worker-class uvicorn.workers.UvicornWorker

# Docker
docker build -t bare-metal-python .
docker run -p 8001:8001 \
  -e PYTHON_SERVICE_DB_URL=postgres://... \
  bare-metal-python
```

---

## TypeScript Frontend Service

### Framework & Setup: Bun

Bun is an all-in-one toolkit for JavaScript/TypeScript development. It's designed to be significantly faster than Node.js for development and production use.

**Key Benefits**:
- Fast bundler and transpiler
- Native TypeScript support
- Excellent performance
- Modern JavaScript APIs

### Service Details

**Location**: `/home/cevor/github/bare_metal_demo/typescript-frontend`
**Language**: TypeScript
**Runtime**: Bun
**Default Port**: 3001 (development), configurable for production
**Package Manager**: Bun

### Project Setup

```json
{
  "name": "bare-metal-frontend",
  "type": "module",
  "scripts": {
    "dev": "bun run --hot src/index.ts",
    "build": "bun build src/index.ts --outdir dist",
    "start": "bun dist/index.js"
  },
  "dependencies": {
    "dotenv": "^16.4.5"
  },
  "devDependencies": {
    "typescript": "^5.3.3",
    "@types/bun": "latest"
  }
}
```

### Running the Service

#### Prerequisites
- Bun installed
- Rust backend running on port 3000
- Python service running on port 8001 (optional)

#### Development Server

```bash
cd typescript-frontend

# Install dependencies (one-time)
bun install

# Start development server with hot reload
bun run dev

# Server runs on http://localhost:3001
```

#### Production Build

```bash
# Build for production
bun run build

# Output directory: dist/

# Run production build
bun run start
# Or: bun dist/index.js
```

### Build Process

**Development Build**:
```bash
bun run --hot src/index.ts
```
- No bundling, files served as-is
- Hot reload on file changes
- Better for development debugging

**Production Build**:
```bash
bun build src/index.ts --outdir dist
```
- Single bundled output file
- Optimized and minified
- Suitable for deployment

### Project Structure

```
typescript-frontend/
├── src/
│   └── index.ts          # Entry point
├── public/               # Static assets
├── package.json         # Project metadata and scripts
├── tsconfig.json        # TypeScript configuration
├── bunfig.toml          # Bun configuration
└── dist/                # Build output (generated)
```

### TypeScript Configuration

**tsconfig.json**:
```json
{
  "compilerOptions": {
    "target": "ES2020",
    "module": "ESNext",
    "lib": ["ES2020"],
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "forceConsistentCasingInFileNames": true
  }
}
```

### Bun Configuration

**bunfig.toml**:
```toml
[build]
root = "src"
outdir = "dist"

[test]
root = "./tests"
```

### API Integration

**Connecting to Rust Backend**:
```typescript
// Example fetch from backend
const response = await fetch('http://localhost:3000/health');
const data = await response.json();
console.log(data); // { status: "ok" }
```

**Connecting to Python Service**:
```typescript
// Example fetch from Python service
const response = await fetch('http://localhost:8001/health');
const data = await response.json();
console.log(data); // { status: "ok" }
```

**CORS Handling**: Both backend services have CORS enabled, so cross-origin requests should work in development.

### Environment Configuration

Create `.env` file in frontend directory:

```env
# API Endpoints
VITE_RUST_BACKEND_URL=http://localhost:3000
VITE_PYTHON_SERVICE_URL=http://localhost:8001

# Environment
NODE_ENV=development
```

### Logging

**Browser Console**:
```typescript
console.log("Debug message");
console.error("Error message");
console.warn("Warning message");
```

**Server Logs**: Bun dev server logs appear in the terminal where you ran `bun run dev`

### Development Tools

```bash
# Format code (can use Prettier)
bun install --dev prettier
bun prettier --write src/

# Lint (can use ESLint)
bun install --dev eslint
bun eslint src/

# Type check
bun tsc --noEmit
```

### Testing

**Add testing framework** (optional):
```bash
bun add --dev bun:test
```

**Run tests**:
```bash
bun test
```

### Building and Deployment

**Development**:
```bash
bun run dev
```

**Production Build**:
```bash
bun run build
bun dist/index.js
```

**Docker**:
```bash
docker build -t bare-metal-frontend .
docker run -p 3001:3001 bare-metal-frontend
```

**Static Hosting** (CDN, S3, etc.):
```bash
# Build outputs to dist/ directory
# Upload contents to static hosting
aws s3 cp dist/ s3://your-bucket/ --recursive
```

---

## Object Storage: Rustfs/MinIO

### Service Details

**Container Image**: rustfs/rustfs:latest
**Default Port**: 9000
**Protocol**: S3-compatible HTTP REST API
**Admin Interface**: http://localhost:9000/minio/ui (may not be available in base image)

### Configuration

**Environment Variables**:
```env
RUSTFS_ROOT_USER=minioadmin
RUSTFS_ROOT_PASSWORD=minioadmin
RUSTFS_ENDPOINT=http://localhost:9000
RUSTFS_ACCESS_KEY=minioadmin
RUSTFS_SECRET_KEY=minioadmin
```

**Connection from Services**:
```python
# Python
import boto3

s3_client = boto3.client(
    's3',
    endpoint_url='http://localhost:9000',
    aws_access_key_id='minioadmin',
    aws_secret_access_key='minioadmin',
    region_name='us-east-1'
)
```

```rust
// Rust (example with rusoto)
use rusoto_s3::{S3Client, S3};
use rusoto_core::Region;

let client = S3Client::new(Region::Custom {
    name: "minio".to_owned(),
    endpoint: "http://localhost:9000".to_owned(),
});
```

### Health Check

```bash
curl http://localhost:9000/minio/health/live
# Returns 200 OK if healthy
```

### Usage Examples

**Create Bucket**:
```bash
# Using AWS CLI
aws s3 mb s3://my-bucket --endpoint-url http://localhost:9000

# Using Python
s3_client.create_bucket(Bucket='my-bucket')
```

**Upload File**:
```bash
# Using AWS CLI
aws s3 cp myfile.txt s3://my-bucket/ --endpoint-url http://localhost:9000

# Using Python
s3_client.upload_file('myfile.txt', 'my-bucket', 'myfile.txt')
```

**Download File**:
```bash
# Using AWS CLI
aws s3 cp s3://my-bucket/myfile.txt . --endpoint-url http://localhost:9000

# Using Python
s3_client.download_file('my-bucket', 'myfile.txt', 'local-file.txt')
```

---

## Service Health Checks

### Complete Health Check Script

```bash
#!/bin/bash

echo "=== Bare Metal Demo Health Check ==="
echo

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m'

check_service() {
    local name=$1
    local url=$2
    
    if curl -s "$url" > /dev/null; then
        echo -e "${GREEN}✓${NC} $name: OK"
        return 0
    else
        echo -e "${RED}✗${NC} $name: DOWN"
        return 1
    fi
}

check_db() {
    local name=$1
    local host=$2
    local port=$3
    local user=$4
    local pass=$5
    local db=$6
    
    if PGPASSWORD=$pass psql -h $host -p $port -U $user -d $db -c "SELECT 1;" > /dev/null 2>&1; then
        echo -e "${GREEN}✓${NC} $name: OK"
        return 0
    else
        echo -e "${RED}✗${NC} $name: DOWN"
        return 1
    fi
}

# Check services
echo "=== API Services ==="
check_service "Rust Backend" "http://localhost:3000/health"
check_service "Python Service" "http://localhost:8001/health"
check_service "Frontend Dev Server" "http://localhost:3001"

echo
echo "=== Databases ==="
check_db "Rust DB" "localhost" "5432" "rust_user" "rust_pass" "rust_service"
check_db "Python DB" "localhost" "5433" "python_user" "python_pass" "python_service"

echo
echo "=== Object Storage ==="
check_service "Rustfs/MinIO" "http://localhost:9000/minio/health/live"

echo
echo "=== Summary ==="
echo "Use 'docker-compose ps' to check container status"
echo "Use 'docker-compose logs [service]' for service logs"
```

### Individual Health Checks

```bash
# Rust Backend
curl http://localhost:3000/health

# Python Service
curl http://localhost:8001/health

# Frontend (HTML response)
curl -I http://localhost:3001

# Rustfs/MinIO
curl http://localhost:9000/minio/health/live

# PostgreSQL (Rust)
PGPASSWORD=rust_pass psql -h localhost -U rust_user -d rust_service -c "SELECT 1;"

# PostgreSQL (Python)
PGPASSWORD=python_pass psql -h localhost -U python_user -d python_service -c "SELECT 1;"
```

---

## Service Communication

### Rust Backend → Python Service

```rust
// From Rust Axum handler
async fn call_python_service() -> Result<String> {
    let response = reqwest::Client::new()
        .get("http://localhost:8001/health")
        .send()
        .await?;
    
    Ok(response.text().await?)
}
```

### Frontend → Rust Backend

```typescript
// From TypeScript
async function getRustHealth() {
    const response = await fetch('http://localhost:3000/health');
    return response.json();
}
```

### Frontend → Python Service

```typescript
// From TypeScript
async function getPythonHealth() {
    const response = await fetch('http://localhost:8001/health');
    return response.json();
}
```

---

## Database Initialization and Migrations

### Automatic Initialization

When Docker containers start, PostgreSQL automatically creates:
- Databases specified in `POSTGRES_DB`
- Users specified in `POSTGRES_USER`

### Manual Initialization (if needed)

```bash
# Connect to Rust database
PGPASSWORD=rust_pass psql -h localhost -U rust_user -d rust_service

# Create tables
CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

# Exit
\q
```

### Running Migration Scripts

```bash
# Rust backend migrations
cd rust-backend
sqlx migrate run

# Python service migrations
cd python-services
alembic upgrade head
```

---

## Next Steps

- See [SETUP.md](./SETUP.md) for installation and troubleshooting
- See [ARCHITECTURE.md](./ARCHITECTURE.md) for system design details
- Check individual service directories for detailed API documentation
