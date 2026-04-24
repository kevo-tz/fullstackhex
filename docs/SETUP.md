# Setup and Installation Guide

## Prerequisites

Before setting up the Bare Metal Demo project, ensure you have the following installed:

### Required Tools

- **Rust** (1.70+): Download from https://rustup.rs/
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  source $HOME/.cargo/env
  ```

- **Python** (3.12): Download from https://www.python.org/downloads/
  ```bash
  python --version  # Verify version
  ```

- **Bun**: Fast JavaScript runtime for TypeScript frontend
  ```bash
  curl -fsSL https://bun.sh/install | bash
  bun --version
  ```

- **Docker**: For running containerized services (PostgreSQL, Rustfs)
  ```bash
  docker --version
  ```

- **Docker Compose**: For orchestrating multi-container setup
  ```bash
  docker-compose --version
  ```

### System Requirements

- **OS**: Linux or macOS (Windows with WSL2 recommended)
- **Memory**: Minimum 2GB RAM, 4GB+ recommended
- **Disk Space**: 2GB free space
- **Ports Available**: 3000, 5432, 5433, 8001, 9000

## Quick Start

### Automated Setup

The easiest way to set up the project is using the automated setup script:

```bash
cd /home/cevor/github/bare_metal_demo
chmod +x scripts/setup.sh
./scripts/setup.sh
```

The setup script will:
1. Install all dependencies (Rust, Python, TypeScript)
2. Start Docker containers for PostgreSQL and Rustfs
3. Initialize databases
4. Build all services
5. Start all services on their designated ports

### Manual Setup (If Automated Setup Fails)

#### 1. Clone the Repository

```bash
git clone <repository-url>
cd bare_metal_demo
```

#### 2. Set Up Environment Variables

```bash
cp .env.example .env
# Edit .env with your local configuration if needed
```

#### 3. Start Docker Services

```bash
# Start PostgreSQL and Rustfs
docker-compose --profile manual up -d

# Verify services are running
docker-compose ps
```

#### 4. Set Up Rust Backend

```bash
cd rust-backend

# Build the project
cargo build

# Run the service
cargo run
# Service will start on http://localhost:3000
```

#### 5. Set Up Python Services

```bash
cd python-services

# Install dependencies using UV (recommended)
uv sync

# Or install using pip
pip install -e .

# Run the service
uv run python -m uvicorn src.main:app --host 0.0.0.0 --port 8001 --reload
# Service will start on http://localhost:8001
```

#### 6. Set Up TypeScript Frontend

```bash
cd typescript-frontend

# Install dependencies
bun install

# Run development server
bun run dev
# Frontend will start on http://localhost:3001
```

## Service Health Checks

### Quick Health Check Script

```bash
#!/bin/bash

echo "Checking Rust Backend..."
curl -s http://localhost:3000/health && echo "" || echo "Rust service down"

echo "Checking Python Service..."
curl -s http://localhost:8001/health && echo "" || echo "Python service down"

echo "Checking Frontend..."
curl -s http://localhost:3001 > /dev/null && echo "Frontend running" || echo "Frontend down"

echo "Checking Rust Database..."
PGPASSWORD=rust_pass psql -h localhost -U rust_user -d rust_service -c "SELECT 1;" > /dev/null && echo "Rust DB OK" || echo "Rust DB down"

echo "Checking Python Database..."
PGPASSWORD=python_pass psql -h localhost -U python_user -d python_service -c "SELECT 1;" > /dev/null && echo "Python DB OK" || echo "Python DB down"

echo "Checking Rustfs..."
curl -s http://localhost:9000/minio/health/live && echo "" || echo "Rustfs down"
```

### Individual Service Health Checks

```bash
# Rust Backend
curl http://localhost:3000/health
# Expected: {"status":"ok"}

# Python Service
curl http://localhost:8001/health
# Expected: {"status":"ok"}

# Rustfs/MinIO
curl http://localhost:9000/minio/health/live
# Expected: {"status":"ok"}
```

## Viewing Logs

### Docker Container Logs

```bash
# Rust database
docker-compose logs -f rust-db

# Python database
docker-compose logs -f python-db

# Rustfs storage
docker-compose logs -f s3-storage
```

### Application Service Logs

For services running locally (not in Docker):

```bash
# Rust Backend (terminal where you ran cargo run)
# Logs appear automatically

# Python Service (terminal where you ran uvicorn)
# Set log level: RUST_LOG=debug

# Frontend
# Bun dev server logs appear in terminal
```

### Log Level Configuration

```bash
# Rust Backend
export RUST_LOG=debug  # Options: error, warn, info, debug, trace
cargo run

# Python Service
# Uvicorn will show access logs by default
```

## Port Mappings Reference

| Service | Port | Description | Protocol |
|---------|------|-------------|----------|
| Rust Backend API | 3000 | Main API service | HTTP |
| Rust PostgreSQL | 5432 | Database for Rust service | PostgreSQL |
| Python PostgreSQL | 5433 | Database for Python service | PostgreSQL |
| Rustfs/MinIO | 9000 | Object storage server | S3-compatible HTTP |
| Python Service | 8001 | FastAPI service | HTTP |
| TypeScript Frontend | 3001 | Development server | HTTP |

## Troubleshooting Common Issues

### Port Already in Use

**Problem**: `Address already in use`

**Solution**:
```bash
# Find process using the port
lsof -i :3000

# Kill the process
kill -9 <PID>

# Or use a different port
cargo run -- --port 3001
```

### Database Connection Failed

**Problem**: `connection refused` or `FATAL: role "rust_user" does not exist`

**Solution**:
```bash
# Check if Docker containers are running
docker-compose ps

# Restart Docker services
docker-compose down
docker-compose --profile manual up -d

# Wait for PostgreSQL to be ready (may take 30 seconds)
docker-compose logs rust-db
```

### Dependency Installation Issues

**Rust**:
```bash
# Update Rust
rustup update

# Clean build
cargo clean
cargo build
```

**Python**:
```bash
# Update UV
pip install --upgrade uv

# Reinstall dependencies
uv sync --refresh
```

**TypeScript/Bun**:
```bash
# Clear Bun cache
bun pm cache rm

# Reinstall
rm -rf node_modules bun.lock
bun install
```

### Environment Variables Not Loading

**Problem**: Services failing with configuration errors

**Solution**:
```bash
# Verify .env file exists
cat .env

# Check syntax
grep -E '^[A-Z_]+=.*' .env

# Make sure it's in the correct directory for each service
# Rust Backend: rust-backend/.env or use export
export RUST_SERVICE_DB_URL=...
```

### Docker Daemon Not Running

**Problem**: `Cannot connect to Docker daemon`

**Solution**:
```bash
# macOS
open --background -a Docker

# Linux
sudo systemctl start docker

# Verify
docker ps
```

### Frontend Cannot Connect to Backend

**Problem**: CORS errors or connection refused

**Solution**:
- Verify Rust backend is running on port 3000
- Check that CORS is enabled (it is by default with `CorsLayer::permissive()`)
- Verify frontend is trying to connect to `http://localhost:3000`

## Performance Tips

1. **Use Release Build for Rust**:
   ```bash
   cargo run --release
   ```

2. **Database Connection Pooling**: Already configured in Rust service (5 connections)

3. **UV for Python**: Much faster than pip for dependency installation

4. **Bun for Frontend**: Significantly faster than npm/yarn

## Next Steps

- See [ARCHITECTURE.md](./ARCHITECTURE.md) for system design overview
- See [SERVICES.md](./SERVICES.md) for detailed service documentation
- Check individual service README files in each service directory
