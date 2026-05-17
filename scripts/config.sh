#!/usr/bin/env bash
# FullStackHex Configuration
# Centralized configuration for all scripts

# Selective exports for subprocesses — only non-sensitive vars exported
# Secrets (POSTGRES_PASSWORD, REDIS_PASSWORD, JWT_SECRET, etc.) passed via
# --env-file .env to docker compose or read directly when needed.

# Guard: .env must exist — all scripts depend on it
if [[ ! -f .env ]]; then
  echo "Error: .env not found. Copy .env.example to .env and fill in required values." >&2
  echo "  cp .env.example .env" >&2
  return 1
fi

# Source common functions
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

# Repository root
REPO_ROOT="$(get_repo_root)"

# Service URLs and ports (can be overridden by environment variables)
RUST_BACKEND_URL="${RUST_BACKEND_URL:-http://localhost:8001}"
FRONTEND_URL="${FRONTEND_URL:-http://localhost:4321}"

# Database configuration
POSTGRES_USER="${POSTGRES_USER:-app_user}"
POSTGRES_PASSWORD="${POSTGRES_PASSWORD:-}"
POSTGRES_DB="${POSTGRES_DB:-app_database}"
POSTGRES_PORT="${POSTGRES_PORT:-5432}"
POSTGRES_HOST="${POSTGRES_HOST:-localhost}"

REDIS_HOST="${REDIS_HOST:-localhost}"
REDIS_PORT="${REDIS_PORT:-6379}"
REDIS_PASSWORD="${REDIS_PASSWORD:-}"

# Benchmark configuration (Apache Bench)
BENCHLITE_REQUESTS="${BENCHLITE_REQUESTS:-1000}"
BENCHLITE_CONCURRENT="${BENCHLITE_CONCURRENT:-100}"

# Performance thresholds (in milliseconds unless otherwise noted)
RUST_HEALTH_P50_THRESHOLD="${RUST_HEALTH_P50_THRESHOLD:-5}"
RUST_HEALTH_P99_THRESHOLD="${RUST_HEALTH_P99_THRESHOLD:-20}"
FRONTEND_TTFB_THRESHOLD="${FRONTEND_TTFB_THRESHOLD:-100}" # in milliseconds

# File paths
BASELINE_DIR="${BASELINE_DIR:-.performance}"
HTML_REPORT_DIR="${HTML_REPORT_DIR:-.performance}"

# Process state
PID_DIR="${PID_DIR:-/tmp/fullstackhex-dev}"
# Fall back to PYTHON_SIDECAR_SOCKET from .env so dev.sh matches Rust's expectation
PYTHON_SOCK="${PYTHON_SOCK:-${PYTHON_SIDECAR_SOCKET:-/tmp/py-api.sock}}"

# Startup timing
POSTGRES_RETRIES="${POSTGRES_RETRIES:-6}"
POSTGRES_POLL_INTERVAL="${POSTGRES_POLL_INTERVAL:-5}"
POST_START_DELAY="${POST_START_DELAY:-2}"

# Docker compose
COMPOSE_DEV="docker compose -f compose/dev.yml --env-file .env"
COMPOSE_MON="docker compose -f compose/monitor.yml --env-file .env -p fullstackhex-monitor"

# Export all variables for use in subshells
export REPO_ROOT
export RUST_BACKEND_URL
export FRONTEND_URL
export POSTGRES_USER
export POSTGRES_DB
export POSTGRES_PORT
export POSTGRES_HOST
export REDIS_HOST
export REDIS_PORT
export BENCHLITE_REQUESTS
export BENCHLITE_CONCURRENT
export RUST_HEALTH_P50_THRESHOLD
export RUST_HEALTH_P99_THRESHOLD
export FRONTEND_TTFB_THRESHOLD
export BASELINE_DIR
export HTML_REPORT_DIR
export PID_DIR
export PYTHON_SOCK
export POSTGRES_RETRIES
export POSTGRES_POLL_INTERVAL
export POST_START_DELAY
export COMPOSE_DEV
export COMPOSE_MON