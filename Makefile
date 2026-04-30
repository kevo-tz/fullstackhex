.PHONY: up down restart logs-backend logs-frontend test bench clean check-env setup setup-env help

# Default values (override with: make up POSTGRES_PASSWORD=mypassword)
COMPOSE_DEV = docker compose -f compose/dev.yml
COMPOSE_PROD = docker compose -f compose/prod.yml
COMPOSE_MON = docker compose -f compose/monitor.yml

# Help
help:
	@echo "FullStackHex - Development Commands"
	@echo ""
	@echo "Setup:"
	@echo "  setup       - First-time setup: install tools + create .env"
	@echo "  setup-env   - Create .env from .env.example (no tool install)"
	@echo ""
	@echo "Services:"
	@echo "  up          - Start all development services"
	@echo "  down        - Stop all services"
	@echo "  restart     - Restart all services"
	@echo ""
	@echo "Logs:"
	@echo "  logs-backend   - Follow Rust backend logs"
	@echo "  logs-frontend  - Follow Astro frontend logs"
	@echo "  logs-db         - Follow PostgreSQL logs"
	@echo "  logs-redis      - Follow Redis logs"
	@echo ""
	@echo "Testing:"
	@echo "  test            - Run all test suites"
	@echo "  test-rust       - Run Rust tests only"
	@echo "  test-python     - Run Python tests only"
	@echo "  test-frontend   - Run frontend tests only"
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
