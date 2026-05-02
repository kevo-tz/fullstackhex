.PHONY: up down restart logs-backend logs-frontend test bench clean check-env setup setup-env help
.PHONY: verify-health check-prereqs preflight down-dev dev watch test-socket-ci

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
	@echo "  test-socket-ci  - Run socket integration tests (CI mode)"
	@echo ""
	@echo "  Example: make test  # runs cargo test + pytest + bun test"
	@echo ""
	@echo "Performance:"
	@echo "  bench           - Run performance benchmarks"
	@echo "  health          - Check all services health"
	@echo "  verify-health   - Poll health endpoints until all OK or timeout"
	@echo "  check-env       - Validate .env has no CHANGE_ME placeholders"
	@echo "  check-prereqs   - Check required dev tools are installed"
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

# check-env: validates .env exists and has no CHANGE_ME placeholders
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

# check-prereqs: detect required dev tools and print install instructions
check-prereqs:
	@echo "Checking prerequisites..."
	@MISSING=0; \
	for tool in bun uv cargo docker; do \
	  if command -v $$tool >/dev/null 2>&1; then \
	    echo "  ✓ $$tool"; \
	  else \
	    echo "  ✗ $$tool — not found"; \
	    MISSING=1; \
	  fi; \
	done; \
	if docker compose version >/dev/null 2>&1; then \
	  echo "  ✓ docker compose"; \
	else \
	  echo "  ✗ docker compose — not found"; \
	  MISSING=1; \
	fi; \
	if [ $$MISSING -eq 1 ]; then \
	  echo ""; \
	  echo "Install missing tools:"; \
	  echo "  bun:   curl -fsSL https://bun.sh/install | bash"; \
	  echo "  uv:    curl -LsSf https://astral.sh/uv/install.sh | sh"; \
	  echo "  cargo: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"; \
	  echo "  docker + compose: https://docs.docker.com/engine/install/"; \
	  exit 1; \
	fi; \
	echo "All prerequisites found."

# preflight: port/socket conflict detection before starting services
preflight:
	@echo "Running preflight checks..."
	@FAIL=0; \
	if ss -tln 2>/dev/null | grep -q ":8001 " || netstat -tln 2>/dev/null | grep -q ".8001 "; then \
	  echo "  ✗ Port 8001 is in use — is another instance running? Run 'make down-dev' first."; \
	  FAIL=1; \
	else \
	  echo "  ✓ Port 8001 free"; \
	fi; \
	if ss -tln 2>/dev/null | grep -q ":4321 " || netstat -tln 2>/dev/null | grep -q ".4321 "; then \
	  echo "  ✗ Port 4321 is in use — is another instance running? Run 'make down-dev' first."; \
	  FAIL=1; \
	else \
	  echo "  ✓ Port 4321 free"; \
	fi; \
	if [ -e "$(PYTHON_SOCK)" ]; then \
	  if ss -xl 2>/dev/null | grep -q "$(PYTHON_SOCK)" || netstat -xl 2>/dev/null | grep -q "$(PYTHON_SOCK)"; then \
	    echo "  ✗ Socket $(PYTHON_SOCK) exists and is in use — is another sidecar running?"; \
	    FAIL=1; \
	  else \
	    echo "  ! Stale socket $(PYTHON_SOCK) detected — unlinking"; \
	    rm -f "$(PYTHON_SOCK)"; \
	    echo "  ✓ Stale socket cleaned up"; \
	  fi; \
	else \
	  echo "  ✓ Socket path free"; \
	fi; \
	if [ $$FAIL -eq 1 ]; then \
	  echo ""; \
	  echo "Preflight failed. Run 'make down-dev' to clean up, then retry."; \
	  exit 1; \
	fi; \
	echo "Preflight passed."

# verify-health: poll health endpoints until all OK or timeout
verify-health:
	@echo "Verifying service health..."
	@TIMEOUT=30; \
	START=$$(date +%s); \
	while true; do \
	  NOW=$$(date +%s); \
	  ELAPSED=$$((NOW - START)); \
	  if [ $$ELAPSED -ge $$TIMEOUT ]; then \
	    echo ""; \
	    echo "Health check timed out after $$TIMEOUT seconds."; \
	    echo ""; \
	    echo "Failing endpoints:"; \
	    curl -sk http://localhost:8001/health 2>/dev/null || echo "  Rust API (8001) — unreachable"; \
	    curl -sk http://localhost:8001/health/db 2>/dev/null || echo "  DB health (8001/health/db) — unreachable"; \
	    curl -sk http://localhost:8001/health/python 2>/dev/null || echo "  Python health (8001/health/python) — unreachable"; \
	    echo ""; \
	    echo "Troubleshooting:"; \
	    echo "  - Is the Rust backend running? Run: cd backend && cargo run -p api"; \
	    echo "  - Is the Python sidecar running? Run: cd python-sidecar && uv run uvicorn app.main:app --uds $(PYTHON_SOCK)"; \
	    echo "  - Is PostgreSQL running? Run: docker compose -f compose/dev.yml up -d postgres"; \
	    exit 1; \
	  fi; \
	  RUST_OK=0; \
	  DB_OK=0; \
	  PY_OK=0; \
	  RUST_STATUS=$$(curl -sk http://localhost:8001/health 2>/dev/null | grep -o '"status":"[^"]*"' | head -1 | cut -d'"' -f4); \
	  if [ "$$RUST_STATUS" = "ok" ]; then RUST_OK=1; fi; \
	  DB_STATUS=$$(curl -sk http://localhost:8001/health/db 2>/dev/null | grep -o '"status":"[^"]*"' | head -1 | cut -d'"' -f4); \
	  if [ "$$DB_STATUS" = "ok" ]; then DB_OK=1; fi; \
	  PY_STATUS=$$(curl -sk http://localhost:8001/health/python 2>/dev/null | grep -o '"status":"[^"]*"' | head -1 | cut -d'"' -f4); \
	  if [ "$$PY_STATUS" = "ok" ]; then PY_OK=1; fi; \
	  printf "."; \
	  if [ $$RUST_OK -eq 1 ] && [ $$DB_OK -eq 1 ] && [ $$PY_OK -eq 1 ]; then \
	    echo ""; \
	    echo "All services healthy ($$ELAPSED s)"; \
	    exit 0; \
	  fi; \
	  sleep 1; \
	done

# dev: full stack with prerequisite checks, preflight, and health verification
dev: check-env check-prereqs preflight
	@trap '$(MAKE) down-dev' INT TERM; \
	$(START_DEPS)
	@echo "Starting Rust backend..."
	cd backend && set -a && . ../.env && set +a && cargo run -p api &
	@echo ""
	@$(MAKE) verify-health
	@echo ""
	@echo "=============================================="
	@echo "  All services healthy. Dashboard:"
	@echo "  → http://localhost:4321"
	@echo "=============================================="
	@echo "Press Ctrl+C to stop everything."
	@wait

watch: check-env check-prereqs preflight
	@command -v cargo-watch >/dev/null 2>&1 || { echo "ERROR: cargo-watch not found. Install: cargo install cargo-watch"; exit 1; }
	@trap '$(MAKE) down-dev' INT TERM; \
	$(START_DEPS)
	@echo "Starting Rust backend (watch mode)..."
	cd backend && set -a && . ../.env && set +a && cargo watch -x 'run -p api' &
	@echo ""
	@$(MAKE) verify-health
	@echo ""
	@echo "=============================================="
	@echo "  All services healthy. Dashboard:"
	@echo "  → http://localhost:4321"
	@echo "=============================================="
	@echo "Press Ctrl+C to stop everything."
	@wait

down-dev:
	@pkill -f "uvicorn app.main:app" 2>/dev/null || true
	@pkill -f "cargo run -p api" 2>/dev/null || true
	@pkill -f "cargo watch" 2>/dev/null || true
	@pkill -f "bun run dev" 2>/dev/null || true
	$(COMPOSE_DEV) down
	@echo "All services stopped."

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
	@echo "Running frontend tests (bun)..."
	cd frontend && bun test
	@echo "Running frontend tests (vitest)..."
	cd frontend && bun run test:vitest

test-socket-ci:
	@echo "Starting test sidecar for socket integration tests..."
	cd python-sidecar && PYTHONUNBUFFERED=1 uv run uvicorn app.main:app --uds /tmp/fullstackhex-test.sock & \
	PID=$$!; \
	sleep 2; \
	echo "Running socket integration tests..."; \
	cd backend && PYTHON_SIDECAR_SOCKET=/tmp/fullstackhex-test.sock cargo test -p python-sidecar -- --ignored; \
	R=$$?; \
	kill $$PID 2>/dev/null || true; \
	rm -f /tmp/fullstackhex-test.sock; \
	exit $$R

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
