.PHONY: up down restart logs-backend logs-frontend test bench clean help

# Default values (override with: make up POSTGRES_PASSWORD=mypassword)
COMPOSE_DEV = docker compose -f compose/dev.yml
COMPOSE_PROD = docker compose -f compose/prod.yml
COMPOSE_MON = docker compose -f compose/monitor.yml

# Help
help:
	@echo "FullStackHex - Development Commands"
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
	@echo ""
	@echo "Cleanup:"
	@echo "  clean           - Reset to fresh state (removes volumes)"
	@echo ""
	@echo "Production:"
	@echo "  prod-up         - Start production stack"
	@echo "  prod-down       - Stop production stack"

# Services
up:
	$(COMPOSE_DEV) up -d
	$(COMPOSE_MON) up -d
	@echo "Services started. Access:"
	@echo "  Frontend: http://localhost:4321"
	@echo "  Backend:  http://localhost:8001"
	@echo "  Grafana:  http://localhost:3000"

down:
	$(COMPOSE_DEV) down
	$(COMPOSE_MON) down

restart: down up

# Logs
logs-backend:
	$(COMPOSE_DEV) logs -f backend

logs-frontend:
	$(COMPOSE_DEV) logs -f frontend

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
