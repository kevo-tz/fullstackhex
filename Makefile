.PHONY: up down restart logs-backend logs-frontend test bench clean check-env sync-env setup setup-env help
.PHONY: verify-health check-prereqs preflight down-dev dev watch test-socket-ci
.PHONY: migrate migrate-revert migrate-status
.PHONY: rollback blue-green canary canary-promote canary-rollback
.PHONY: logs-db logs-redis deploy-check prod-restart prod-up prod-down
.PHONY: status run-dev check-cargo-watch status-sh

# Default values (override with: make up POSTGRES_PASSWORD=mypassword)
COMPOSE_DEV = docker compose -f compose/dev.yml --env-file .env
COMPOSE_PROD = docker compose -f compose/prod.yml --env-file .env
COMPOSE_MON = docker compose -f compose/monitor.yml --env-file .env -p fullstackhex-monitor

PYTHON_TEST_SOCK ?= /tmp/fullstackhex-test.sock
PID_DIR = /tmp/fullstackhex-dev
POST_START_DELAY ?= 2

# PostgreSQL readiness tuning
POSTGRES_RETRIES ?= 6
POSTGRES_POLL_INTERVAL ?= 5
PYTHON_SOCK ?= /tmp/fullstackhex-python.sock

# Shared startup sequence used by dev and watch targets
START_DEPS = \
	$(COMPOSE_DEV) up -d; \
	echo "Waiting for PostgreSQL (up to $$(( $(POSTGRES_RETRIES) * $(POSTGRES_POLL_INTERVAL) ))s)..."; \
	for i in $$(seq 1 $(POSTGRES_RETRIES)); do \
	  docker compose -f compose/dev.yml exec -T postgres pg_isready -U app_user 2>/dev/null && break; \
	  sleep $(POSTGRES_POLL_INTERVAL); \
	done; \
	echo "PostgreSQL ready (or timeout — Rust will retry connections)"; \
	echo "Starting Python sidecar..."; \
	cd python-sidecar && set -a && . ../.env && set +a && uv run uvicorn app.main:app --uds $(PYTHON_SOCK) & \
	echo $$! > $(PID_DIR)/python.pid; \
	echo "Starting frontend..."; \
	cd frontend && bun run dev & \
	echo $$! > $(PID_DIR)/frontend.pid

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
	@echo "Database:"
	@echo "  migrate         - Run pending database migrations"
	@echo "  migrate-revert  - Revert last database migration"
	@echo "  migrate-status  - Show migration status"
	@echo ""
	@echo "Services:"
	@echo "  up          - Start all development services (infra only)"
	@echo "  dev         - Start full stack (infra + python + rust + frontend)"
	@echo "  watch       - Start full stack with Rust hot reload (cargo watch)"
	@echo "              Ctrl+C stops all. For persistent per-service startup, see README."
	@echo "  down        - Stop all services"
	@echo "  status      - Show which services are running (PID, port, health)"
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
	@echo "  check-env       - Validate .env has all required keys and no CHANGE_ME placeholders"
	@echo "  sync-env        - Compare .env against .env.example, show missing keys"
	@echo "  sync-env-apply  - Append missing keys to .env (commented out)"
	@echo "  check-prereqs   - Check required dev tools are installed"
	@echo ""
	@echo "Cleanup:"
	@echo "  clean           - Reset to fresh state (removes volumes)"
	@echo ""
	@echo "Production:"
	@echo "  prod-up         - Start production stack"
	@echo "  prod-down       - Stop production stack"
	@echo "  deploy          - Deploy to VPS (SSH + rsync + docker compose)"
	@echo "  rollback        - Rollback to previous version"
	@echo "  blue-green      - Zero-downtime blue-green deployment"
	@echo "  canary          - Canary deployment (10% traffic)"
	@echo "  canary-promote  - Promote canary to primary"
	@echo "  canary-rollback - Rollback canary"

# Setup
setup: ## First-time setup: install dev tools and create .env
	./scripts/install-deps.sh
	@if [ ! -f .env ]; then cp .env.example .env && echo ".env created — review it before running make up"; fi

setup-env: ## Create .env from .env.example (skips tool installation)
	./scripts/setup-env.sh

# Database migrations
migrate: ## Run pending database migrations
	@echo "Running database migrations..."
	@cd backend && cargo sqlx migrate run
	@echo "Migrations applied."

migrate-revert: ## Revert last database migration
	@echo "Reverting last migration..."
	@cd backend && cargo sqlx migrate revert
	@echo "Migration reverted."

migrate-status: ## Show database migration status
	@cd backend && cargo sqlx migrate info

# Services

# check-env: validates .env exists, has no CHANGE_ME placeholders,
# all required keys from .env.example are present, and no shell syntax errors.
check-env:
	@./scripts/validate-env.sh

# sync-env: compare .env against .env.example and report missing keys.
# With --apply: append missing keys (commented out) to .env.
sync-env:
	@./scripts/sync-env.sh

sync-env-apply:
	@./scripts/sync-env.sh --apply

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
	    curl -sk --max-time 5 http://localhost:8001/health 2>/dev/null || echo "  Rust API (8001) — unreachable"; \
	    curl -sk --max-time 5 http://localhost:8001/health/db 2>/dev/null || echo "  DB health (8001/health/db) — unreachable"; \
	    curl -sk --max-time 5 http://localhost:8001/health/python 2>/dev/null || echo "  Python health (8001/health/python) — unreachable"; \
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
	  RUST_STATUS=$$(curl -sk --max-time 5 http://localhost:8001/health 2>/dev/null | grep -o '"status":"[^"]*"' | head -1 | sed 's/"status":"//;s/"//' 2>/dev/null); \
	  if [ "$$RUST_STATUS" = "ok" ]; then RUST_OK=1; fi; \
	  DB_STATUS=$$(curl -sk --max-time 5 http://localhost:8001/health/db 2>/dev/null | grep -o '"status":"[^"]*"' | head -1 | sed 's/"status":"//;s/"//' 2>/dev/null); \
	  if [ "$$DB_STATUS" = "ok" ]; then DB_OK=1; fi; \
	  PY_STATUS=$$(curl -sk --max-time 5 http://localhost:8001/health/python 2>/dev/null | grep -o '"status":"[^"]*"' | head -1 | sed 's/"status":"//;s/"//' 2>/dev/null); \
	  if [ "$$PY_STATUS" = "ok" ]; then PY_OK=1; fi; \
	  printf "."; \
	  if [ $$RUST_OK -eq 1 ] && [ $$DB_OK -eq 1 ] && [ $$PY_OK -eq 1 ]; then \
	    echo ""; \
	    echo "All services healthy ($$ELAPSED s)"; \
	    exit 0; \
	  fi; \
	  sleep 1; \
	done

# run-dev: shared startup sequence used by dev and watch targets.
# Set BACKEND_CMD before invoking (dev uses cargo run, watch uses cargo watch).
run-dev: check-env check-prereqs preflight
	@mkdir -p $(PID_DIR); \
	rm -f $(PID_DIR)/*.pid; \
	trap '$(MAKE) down-dev' INT TERM; \
	$(START_DEPS); \
	echo "Starting Rust backend (nohup — survives terminal close)..."; \
	cd backend && set -a && . ../.env && set +a && nohup $(BACKEND_CMD) > $(PID_DIR)/backend.log 2>&1 & \
	echo $$! > $(PID_DIR)/backend.pid; \
	sleep $(POST_START_DELAY); \
	$(MAKE) verify-health; \
	echo ""; \
	echo "=============================================="; \
	echo "  All services healthy. Dashboard:"; \
	echo "  → http://localhost:4321"; \
	echo "=============================================="; \
	echo "Press Ctrl+C to stop Docker services (frontend + sidecar keep running)."; \
	echo "Backend logs: $(PID_DIR)/backend.log"; \
	wait

dev: BACKEND_CMD = cargo run -p api
dev: run-dev

watch: BACKEND_CMD = cargo watch -x 'run -p api'
watch: check-cargo-watch run-dev

check-cargo-watch:
	@command -v cargo-watch >/dev/null 2>&1 || { echo "ERROR: cargo-watch not found. Install: cargo install cargo-watch"; exit 1; }

down-dev:
	@for pidfile in $(PID_DIR)/*.pid; do \
	  if [ -f "$$pidfile" ]; then \
	    read pid < "$$pidfile" 2>/dev/null || true; \
	    kill $$pid 2>/dev/null || true; \
	    rm -f "$$pidfile"; \
	  fi; \
	done
	@pkill -x uvicorn 2>/dev/null || true
	@pkill -x api 2>/dev/null || true
	@pkill -x bun 2>/dev/null || true
	# pkill above is a safety net for orphaned processes where PID files were lost
	# (e.g., after a crash). Primary cleanup is via PID file loop above.
	$(COMPOSE_DEV) down
	@echo "All services stopped."

status:
	@./scripts/status.sh

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
	cd python-sidecar && PYTHONUNBUFFERED=1 uv run uvicorn app.main:app --uds $(PYTHON_TEST_SOCK) & \
	PID=$$!; \
	for i in $$(seq 1 20); do \
	  test -S $(PYTHON_TEST_SOCK) && break; \
	  sleep 0.3; \
	done; \
	echo "Running socket integration tests..."; \
	cd backend && PYTHON_SIDECAR_SOCKET=$(PYTHON_TEST_SOCK) cargo test -p python-sidecar -- --ignored; \
	R=$$?; \
	kill $$PID 2>/dev/null || true; \
	rm -f $(PYTHON_TEST_SOCK); \
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

# Deployment to VPS via SSH + rsync
# Requires: ssh-agent with key loaded, .env with DEPLOY_HOST, DEPLOY_USER, DEPLOY_PATH
deploy: check-env
	@echo "Deploying to $(DEPLOY_HOST)..."
	@test -n "$(DEPLOY_HOST)" || { echo "ERROR: DEPLOY_HOST not set in .env"; exit 1; }
	@test -n "$(DEPLOY_USER)" || { echo "ERROR: DEPLOY_USER not set in .env"; exit 1; }
	@test -n "$(DEPLOY_PATH)" || { echo "ERROR: DEPLOY_PATH not set in .env"; exit 1; }
	rsync -avz --exclude='.git' --exclude='target' --exclude='node_modules' \
	  compose/ nginx/ scripts/ Makefile .env \
	  $(DEPLOY_USER)@$(DEPLOY_HOST):$(DEPLOY_PATH)/
	ssh $(DEPLOY_USER)@$(DEPLOY_HOST) "cd $(DEPLOY_PATH) && chmod 600 .env && docker compose -f compose/prod.yml up -d --wait"
	$(MAKE) deploy-check

# Post-deploy health check: poll remote /health until OK or timeout
deploy-check:
	@echo "Checking remote health at $(DEPLOY_HOST)..."
	@TIMEOUT=60; \
	START=$$(date +%s); \
	while true; do \
	  NOW=$$(date +%s); \
	  ELAPSED=$$((NOW - START)); \
	  if [ $$ELAPSED -ge $$TIMEOUT ]; then \
	    echo ""; \
	    echo "Deploy health check timed out after $$TIMEOUT seconds."; \
	    exit 1; \
	  fi; \
	  CURL_FLAGS="--max-time 5"; \
	  RESP=$$(curl -sS $$CURL_FLAGS "https://$(DEPLOY_HOST)/health" 2>/dev/null); \
	  if [ -z "$$RESP" ]; then \
	    RESP=$$(curl -sk $$CURL_FLAGS "https://$(DEPLOY_HOST)/health" 2>/dev/null); \
	  fi; \
	  STATUS=$$(echo "$$RESP" | grep -o '"status":"[^"]*"' | head -1 | sed 's/"status":"//;s/"//' 2>/dev/null); \
	  if [ "$$STATUS" = "ok" ]; then \
	    echo ""; \
	    echo "Remote health OK ($$ELAPSED s)"; \
	    exit 0; \
	  fi; \
	  printf "."; \
	  sleep 1; \
	done

# Restart production stack (pulls latest images)
prod-restart:
	@test -n "$(DEPLOY_HOST)" || { echo "ERROR: DEPLOY_HOST not set in .env"; exit 1; }
	@test -n "$(DEPLOY_USER)" || { echo "ERROR: DEPLOY_USER not set in .env"; exit 1; }
	@test -n "$(DEPLOY_PATH)" || { echo "ERROR: DEPLOY_PATH not set in .env"; exit 1; }
	ssh $(DEPLOY_USER)@$(DEPLOY_HOST) "cd $(DEPLOY_PATH) && docker compose -f compose/prod.yml down && docker compose -f compose/prod.yml up -d --wait"

# Deploy safety — rollback, blue-green, canary
rollback: ## Rollback to the previous deployment version
	./scripts/rollback.sh

blue-green: ## Zero-downtime blue-green deployment
	./scripts/deploy-blue-green.sh

canary: ## Canary deployment (10% traffic to new version)
	./scripts/deploy-canary.sh

canary-promote: ## Promote canary to primary (100% traffic)
	./scripts/deploy-canary-promote.sh

canary-rollback: ## Rollback canary deployment
	./scripts/deploy-canary-rollback.sh
