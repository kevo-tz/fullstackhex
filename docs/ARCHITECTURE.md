# System Architecture Overview

## High-Level System Design

The Bare Metal Demo is a distributed system composed of multiple independent microservices that communicate via HTTP APIs. The system is designed for demonstration and development purposes, showcasing modern polyglot development with Rust, Python, and TypeScript.

### System Overview

```
┌─────────────────────────────────────────────────────────────────────────┐
│                        Client Layer                                     │
├─────────────────────────────────────────────────────────────────────────┤
│                   TypeScript Frontend (Port 3001)                       │
│              (Bun runtime, development server mode)                     │
└─────────────────────────────┬───────────────────────────────────────────┘
                              │
                    ┌─────────┴────────┐
                    │                  │
        ┌───────────▼──────────┐  ┌───▼──────────────────┐
        │                      │  │                      │
┌───────┴────────────────────┐ │ │ ┌────────────────────┴──┐
│  Rust Backend (Port 3000)  │ │ │ │ Python Service      │
│  Framework: Axum           │ │ │ │ (Port 8001)         │
│  Runtime: Tokio            │ │ │ │ Framework: FastAPI  │
└──────────┬─────────────────┘ │ │ │ Runtime: uvicorn   │
           │                   │ │ │                     │
           │        ┌──────────┴─┴─┴─────────────────────┘
           │        │
    ┌──────▼────────▼───────────────────────────────┐
    │         Database Layer                        │
    ├─────────────────────────────────────────────────┤
    │ Rust Service PostgreSQL (Port 5432)           │
    │ Database: rust_service                        │
    │ User: rust_user                               │
    │                                               │
    │ Python Service PostgreSQL (Port 5433)         │
    │ Database: python_service                      │
    │ User: python_user                             │
    └─────────────────────────────────────────────────┘
    
    ┌──────────────────────────────────────────────────┐
    │    Object Storage Layer (Port 9000)             │
    │  Rustfs/MinIO - S3-compatible storage          │
    │  User: minioadmin                              │
    └──────────────────────────────────────────────────┘
```

## Data Flow

### Request Flow

1. **Frontend Request**: TypeScript frontend makes HTTP request to backend
2. **Backend Processing**: Rust backend (Axum) handles request, queries database if needed
3. **Database Operation**: Backend executes SQL queries via SQLx connection pool
4. **Response Return**: JSON response sent back to frontend
5. **Optional**: Python service called for specific operations (optional, depends on endpoints)

### Service Communication

```
Frontend  ──HTTP──>  Rust Backend  ────HTTP────>  Python Service
   │                       │                             │
   │                       │                             │
   └───────────────────────┴─────────────────────────────┘
                           │
                     ┌─────┴──────┐
                     │            │
                PostgreSQL    PostgreSQL
                (Rust DB)     (Python DB)
                     │            │
                     └─────┬──────┘
                           │
                    Rustfs/MinIO
                     (S3 Storage)
```

## Technology Stack

### Rust Backend
- **Framework**: Axum (async web framework)
- **Runtime**: Tokio (async execution)
- **Database**: PostgreSQL with sqlx (compile-time query checking)
- **Dependencies**:
  - `axum`: Web server
  - `tokio`: Async runtime
  - `sqlx`: Database driver
  - `tower-http`: Middleware (CORS, tracing)
  - `serde`: Serialization/deserialization
  - `chrono`: Date/time handling
  - `uuid`: Unique identifiers

### Python Services
- **Framework**: FastAPI (modern, fast Python web framework)
- **ASGI Server**: uvicorn (production-grade ASGI server)
- **Database**: PostgreSQL with psycopg (async-capable Python driver)
- **Dependencies**:
  - `fastapi`: Web framework
  - `uvicorn`: ASGI server
  - `psycopg`: PostgreSQL driver
  - `pydantic`: Data validation
  - `python-dotenv`: Environment configuration

### TypeScript Frontend
- **Runtime**: Bun (fast JavaScript runtime)
- **Language**: TypeScript
- **Build Tool**: Bun build
- **Dev Server**: Bun dev server with hot reload

### Data Layer
- **Primary Databases**: PostgreSQL 16 (Alpine)
  - Two separate instances: one for Rust service, one for Python service
  - Connection pooling configured for performance
  - Health checks enabled
  
- **Object Storage**: Rustfs/MinIO
  - S3-compatible API
  - RESTful object storage
  - Optional for file/blob storage needs

## Database Schema Overview

### Rust Service Database (`rust_service`)

Connection: `postgresql://rust_user:rust_pass@localhost:5432/rust_service`

**Typical Tables**:
- Services may include user data, business domain tables
- All tables use serial or UUID primary keys
- Timestamps for audit trails (created_at, updated_at)

**Example Setup**:
```sql
-- Run migrations if available
psql -h localhost -U rust_user -d rust_service -f migrations/001_initial.sql
```

### Python Service Database (`python_service`)

Connection: `postgresql://python_user:python_pass@localhost:5433/python_service`

**Typical Tables**:
- Service-specific domain data
- Isolated from Rust service database
- Same table design patterns as Rust service

**Example Setup**:
```sql
-- Run migrations if available
psql -h localhost -U python_user -d python_service -f migrations/001_initial.sql
```

## Port Mappings Architecture

```
External World (Your Computer)
         │
         │
    ┌────▼────────────────────────────────────────┐
    │        Port Mapping (localhost)              │
    ├─────────────────────────────────────────────┤
    │ 3000 ──>  Rust Backend (Axum)               │
    │ 3001 ──>  TypeScript Frontend (Bun)         │
    │ 8001 ──>  Python Service (FastAPI)          │
    │ 5432 ──>  Rust PostgreSQL Database          │
    │ 5433 ──>  Python PostgreSQL Database        │
    │ 9000 ──>  Rustfs/MinIO Object Storage       │
    └──────────────────────────────────────────────┘
```

## Service Isolation

Each service is designed to be:

1. **Independent**: Can run, test, and deploy separately
2. **Isolated**: Own database instance
3. **Loosely Coupled**: Communication only via HTTP APIs
4. **Scalable**: Each can scale independently

### Rust Backend Isolation
- Dedicated PostgreSQL instance
- Optional connection to Rustfs
- HTTP API for frontend/other services

### Python Service Isolation
- Dedicated PostgreSQL instance
- Optional connection to Rustfs
- Separate HTTP API endpoint
- Can be called by Rust backend if needed

### Frontend Isolation
- Communicates only via HTTP
- No direct database access
- Development-focused setup (Bun dev server)

## Deployment Topologies

### Development (Current)
```
All services running locally on different ports
Databases in Docker containers
Suitable for: Local development, testing
```

### Docker Compose (Current Setup)
```
Services can run in containers
Databases containerized
Volumes for persistent data
Networks for service-to-service communication
```

### Production (Example)
```
Each service on separate VM/container
Managed PostgreSQL (RDS, Cloud SQL, etc.)
CDN for frontend
Load balancing for APIs
```

## Environment Configuration

All services read from `.env` file:

```env
# Rust Backend
RUST_SERVICE_DB_URL=postgres://rust_user:rust_pass@localhost:5432/rust_service

# Python Services
PYTHON_SERVICE_DB_URL=postgres://python_user:python_pass@localhost:5433/python_service

# Rustfs
RUSTFS_ENDPOINT=http://localhost:9000
RUSTFS_ACCESS_KEY=minioadmin
RUSTFS_SECRET_KEY=minioadmin

# Logging
RUST_LOG=info

# Environment
NODE_ENV=development
```

## Scaling Considerations

### Horizontal Scaling
- Frontend: Serve from multiple servers behind load balancer
- Rust Backend: Deploy multiple instances with load balancer
- Python Services: Deploy multiple instances with load balancer
- Databases: Use replication and read replicas

### Vertical Scaling
- Increase database connection pool sizes
- Increase worker threads in Tokio runtime
- Allocate more memory to Python service

### Caching Strategy
- Frontend: Browser cache, HTTP cache headers
- Backends: Can add Redis layer between services and database
- Database query results: Application-level caching

## Security Considerations

### Current Development Setup
- No authentication/authorization on endpoints
- CORS permissively enabled
- Default credentials for databases

### Production Hardening
- Implement JWT or OAuth2 authentication
- Restrict CORS to specific domains
- Use strong database passwords
- Enable SSL/TLS for all connections
- Implement rate limiting
- Add API key authentication
- Use environment-specific secrets management

## Next Steps

- See [SETUP.md](./SETUP.md) for installation instructions
- See [SERVICES.md](./SERVICES.md) for detailed service documentation
- Review individual service source code for API specifications
