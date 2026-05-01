.PHONY: up down restart logs-backend logs-frontend test bench clean check-env setup setup-env help

# Default values (override with: make up POSTGRES_PASSWORD=mypassword)
COMPOSE_DEV = docker compose -f compose/dev.yml --env-file .env
COMPOSE_PROD = docker compose -f compose/prod.yml --env-file .env
COMPOSE_MON = docker compose -f compose/monitor.yml --env-file .env

# PostgreSQL readiness tuning
POSTGRES_RETRIES ?= 6
POSTGRES_POLL_INTERVAL ?= 5
PYTHON_SOCK ?= /tmp/fullstackhex-python.sock

# Shared startup sequence used by dev and watch targets
START_DEPS = \
	$(COMPOSE_DEV) up -d; \
	@echo "Waiting for PostgreSQL (up to $$(( $(POSTGRES_RETRIES) * $(POSTGRES_POLL_INTERVAL) ))s)..."; \
	@for i in $$(seq 1 $(POSTGRES_RETRIES)); do \
	  docker compose -f compose/dev.yml exec -T postgres pg_isready -U app_user 2>/dev/null && break; \
	  sleep $(POSTGRES_POLL_INTERVAL); \
	done; \
	@echo "PostgreSQL ready (or timeout — Rust will retry connections)"; \
	@echo "Starting Python sidecar..."; \
	cd python-sidecar && set -a && . ../.env && set +a && uv run uvicorn app.main:app --uds $(PYTHON_SOCK) &; \
	@echo "Starting frontend..."; \
	cd frontend && bun run dev &

# Help
dev: check-env
	$(START_DEPS)
	@echo "Starting Rust backend..."
	cd backend && set -a && . ../.env && set +a && cargo run -p api &
	@echo ""
	@echo "All services starting. Dashboard at http://localhost:4321"
	@echo "Run 'make down-dev' to stop everything."

watch: check-env
	@command -v cargo-watch >/dev/null 2>&1 || { echo "ERROR: cargo-watch not found. Install: cargo install cargo-watch"; exit 1; }
	$(START_DEPS)
	@echo "Starting Rust backend (watch mode)..."
	cd backend && set -a && . ../.env && set +a && cargo watch -x 'run -p api' &
	@echo ""
	@echo "All services starting with hot reload. Dashboard at http://localhost:4321"
	@echo "Run 'make down-dev' to stop everything."

down-dev:
	@pkill -f "uvicorn app.main:app" 2>/dev/null || true
	@pkill -f "cargo run -p api" 2>/dev/null || true
	@pkill -f "cargo watch" 2>/dev/null || true
	@pkill -f "bun run dev" 2>/dev/null || true
	$(COMPOSE_DEV) down
	@echo "All services stopped."

help:
	@echo "FullStackHex - Development Commands"
	@echo ""
	@echo "Setup:"
	@echo "  setup       - First-time setup: install tools + create .env"
	@echo "  setup-env   - Create .env from .env.example (no tool install)"
	@echo ""
	@echo "  Example: make setup && make dev"
	@echo ""
	@echo "Services:"
	@echo "  up          - Start all development services (infra only)"
	@echo "  dev         - Start full stack (infra + python + rust + frontend)"
	@echo "  watch       - Start full stack with Rust hot reload (cargo watch)"
	@echo "  down        - Stop all services"
	@echo "  down-dev    - Stop full stack and infrastructure"
	@echo "  restart     - Restart all services"
	@echo ""
	@echo "  Example: make watch # starts everything with live reload"
	@echo ""
	@echo "Logs:"
	@echo "  logs-backend   - Follow Rust backend logs"
	@echo "  logs-frontend  - Follow Astro frontend logs"
	@echo "  logs-python    - Python sidecar log guidance"
	@echo "  logs-db        - Follow PostgreSQL logs"
	@echo "  logs-redis     - Follow Redis logs"
	@echo ""
	@echo "Testing:"
	@echo "  test            - Run all test suites"
	@echo "  test-rust       - Run Rust tests only"
	@echo "  test-python     - Run Python tests only"
	@echo "  test-frontend   - Run frontend tests only"
	@echo ""
	@echo "  Example: make test  # runs cargo test + pytest + bun test"
	@echo ""
	@echo "Performance:"
	@echo "  bench           - Run performance benchmarks"
	@echo "  health          - Check all services health"
	@echo "  check-env       - Validate .env has no CHANGE_ME placeholders"
	@echo ""
	@echo "Cleanup:"
	@echo "  clean           - Reset to fresh state (removes volumes)"
	@echo ""
	@echo "Production:"
	@echo "  prod-up         - Start production stack"
	@echo "  prod-down       - Stop production stack"

# Setup
setup: ## First-time setup: install dev tools and create .env
	./scripts/install-deps.sh
	@if [ ! -f .env ]; then cp .env.example .env && echo ".env created — review it before running make up"; fi

setup-env: ## Create .env from .env.example (skips tool installation)
	./scripts/setup-env.sh

# Services
check-env:
	@if [ ! -f .env ]; then \
	  echo "ERROR: .env not found. Run: cp .env.example .env"; \
	  exit 1; \
	fi
	@if grep -q "CHANGE_ME" .env 2>/dev/null; then \
	  echo "ERROR: .env still contains CHANGE_ME placeholder values."; \
	  echo "       Edit .env and replace all CHANGE_ME entries before continuing."; \
	  grep -n "CHANGE_ME" .env; \
	  exit 1; \
	fi
	@echo ".env looks good."

up: check-env
	$(COMPOSE_DEV) up -d
	$(COMPOSE_MON) up -d
	@echo "Infrastructure services started. To run the app:"
	@echo "  Backend:  cd backend && cargo run -p api"
	@echo "  Frontend: cd frontend && bun run dev"
	@echo "  Grafana:  http://localhost:3000"

down:
	$(COMPOSE_DEV) down
	$(COMPOSE_MON) down

restart: down up

# Logs
logs-backend:
	@echo "Rust backend runs directly (not in compose)."
	@echo "  Follow logs with: cd backend && cargo run -p api 2>&1 | tee backend.log"

logs-frontend:
	@echo "Astro frontend runs directly (not in compose)."
	@echo "  Follow logs with: cd frontend && bun run dev"

logs-db:
	$(COMPOSE_DEV) logs -f postgres

logs-redis:
	$(COMPOSE_DEV) logs -f redis

logs-python:
	@echo "Python sidecar runs directly (not in compose)."
	@echo "  View logs in the terminal where 'make dev' or 'make watch' is running."
	@echo "  Or restart manually with: cd python-sidecar && uv run uvicorn app.main:app --uds $(PYTHON_SOCK)"

# Testing
test: test-rust test-python test-frontend

test-rust:
	@echo "Running Rust tests..."
	cd backend && cargo test --workspace

test-python:
	@echo "Running Python tests..."
	cd python-sidecar && uv run pytest

test-frontend:
	@echo "Running frontend tests..."
	cd frontend && bun test

# Performance
bench:
	./scripts/bench.sh

health:
	./scripts/verify-health.sh --verbose

# Cleanup
clean:
	$(COMPOSE_DEV) down -v --remove-orphans
	$(COMPOSE_MON) down -v --remove-orphans
	@echo "Cleaned up all services and volumes"

# Production
prod-up:
	$(COMPOSE_PROD) up -d
	@echo "Production stack started. Access via nginx."

prod-down:
	$(COMPOSE_PROD) down
