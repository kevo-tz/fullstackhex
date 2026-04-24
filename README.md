# Bare Metal Demo

A full-stack application demonstrating a complete development setup with a **Rust backend**, **Python services**, and **TypeScript frontend**, all orchestrated with containerized infrastructure.

## Architecture Overview

```
┌─────────────────────┐
│  TypeScript Frontend │  (Bun, React, TailwindCSS)
│      Port 3000      │
└──────────┬──────────┘
           │
           ├─────────────────────┬──────────────────┐
           │                     │                  │
┌──────────▼──────────┐  ┌──────▼────────┐  ┌────▼──────────┐
│   Rust Backend      │  │Python Services│  │ RustFS (S3)   │
│   Port 8001         │  │  Ports 8000   │  │ Port 9000     │
│ (Actix Web)         │  │  (FastAPI)    │  │ (MinIO compat)│
└──────────┬──────────┘  └──────┬────────┘  └───────────────┘
           │                    │
           ├────────────┬───────┘
           │            │
      ┌────▼────┐  ┌───▼──────┐
      │Rust DB  │  │ Python DB│
      │:5432    │  │  :5433   │
      └─────────┘  └──────────┘
```

## Components

### **Rust Backend** (`rust-backend/`)
- **Framework:** Actix Web
- **Database:** PostgreSQL (port 5432)
- **Port:** 8001
- **Files:**
  - `Cargo.toml` - Project manifest
  - `src/main.rs` - Entry point
  - `migrations/` - Database migration files

### **Python Services** (`python-services/`)
- **Framework:** FastAPI
- **Database:** PostgreSQL (port 5433)
- **Port:** 8000
- **Files:**
  - `pyproject.toml` - Project configuration with uv package manager
  - `uv.lock` - Dependency lock file
  - `src/main.py` - Entry point

### **TypeScript Frontend** (`typescript-frontend/`)
- **Runtime:** Bun
- **Framework:** React
- **Styling:** TailwindCSS
- **Port:** 3000
- **Files:**
  - `package.json` - Project manifest
  - `bunfig.toml` - Bun configuration
  - `tsconfig.json` - TypeScript configuration
  - `src/index.ts` - Entry point

### **Infrastructure** (`docker-compose.yml`)
- **Postgres (Rust):** postgres:16-alpine → localhost:5432
- **Postgres (Python):** postgres:16-alpine → localhost:5433
- **RustFS (S3-compatible):** rustfs/rustfs:latest → localhost:9000

## Quick Start

### Prerequisites
- Docker & Docker Compose
- Rust (for backend development)
- Python 3.9+
- Bun (for frontend)

### Setup

```bash
# Run the setup script
./scripts/setup.sh

# Verify the setup
./scripts/verify-setup.sh
```

The setup script will:
1. Install all dependencies
2. Start Docker containers (Postgres, RustFS)
3. Run database migrations
4. Configure environment variables

### Running Services

```bash
# Start all containers in background
docker-compose up -d

# Check container status
docker-compose ps

# View logs
docker-compose logs -f

# Stop containers
docker-compose down
```

### Development

**Rust Backend:**
```bash
cd rust-backend
cargo run
```

**Python Services:**
```bash
cd python-services
uv run python src/main.py
```

**TypeScript Frontend:**
```bash
cd typescript-frontend
bun install
bun run dev
```

## Configuration

### Environment Variables
Copy `.env.example` to `.env` and update as needed:

```bash
cp .env.example .env
```

Key variables:
- `RUST_SERVICE_DB_URL` - Rust service database connection
- `PYTHON_SERVICE_DB_URL` - Python service database connection
- `RUSTFS_ENDPOINT` - S3-compatible storage endpoint
- `NODE_ENV` - Frontend environment (development/production)

## Documentation

For detailed information, see:
- **[SETUP.md](docs/SETUP.md)** - Detailed setup and troubleshooting
- **[ARCHITECTURE.md](docs/ARCHITECTURE.md)** - System architecture and design
- **[SERVICES.md](docs/SERVICES.md)** - Service descriptions and APIs

## Cleanup

To remove all containers and data:

```bash
./scripts/cleanup.sh
```

## Project Structure

```
bare_metal_demo/
├── rust-backend/          # Rust service with Actix Web
├── python-services/       # Python service with FastAPI
├── typescript-frontend/   # Frontend with Bun & React
├── scripts/              # Setup, verification, and cleanup scripts
├── docs/                 # Architecture and setup documentation
├── docker-compose.yml    # Docker container orchestration
├── .env.example          # Environment variable template
└── README.md            # This file
```

## Technologies

| Component | Technology |
|-----------|-----------|
| Backend | Rust, Actix Web |
| Services | Python, FastAPI |
| Frontend | TypeScript, Bun, React, TailwindCSS |
| Databases | PostgreSQL 16 |
| Storage | RustFS (MinIO-compatible S3) |
| Orchestration | Docker Compose |

## Notes

- Postgres databases run in Docker containers locally
- RustFS provides S3-compatible object storage in Docker
- Application services can run locally or in containers
- See `.env.example` for all configuration options

## License

See LICENSE file for details.
