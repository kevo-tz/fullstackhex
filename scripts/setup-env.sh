#!/bin/bash
# FullStackHex Environment Setup
# Sets up environment variables and configuration files

# Source common functions and configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"
source "$SCRIPT_DIR/config.sh"

log_info "Setting up environment..."

# Copy .env if not exists
if [ ! -f .env ]; then
    if [ -f .env.example ]; then
        cp .env.example .env
        log_success "Created .env from .env.example"
    else
        touch .env
        log_warning ".env.example not found, created empty .env"
    fi
else
    log_success ".env already exists"
fi

# Add Unix socket path to .env if not present
if ! grep -q "PYTHON_SIDECAR_SOCKET" .env 2>/dev/null; then
    # CI environments get a temp path; local gets user-isolated path
    if [ "${CI:-false}" = "true" ]; then
        local socket_dir="${RUNNER_TEMP:-$PWD/.tmp}/sockets"
        mkdir -p "$socket_dir"
        local socket_path="$socket_dir/python-sidecar.sock"
        log_warning "CI detected: using temp socket path"
    else
        local socket_dir="$HOME/.fullstackhex/sockets"
        mkdir -p "$socket_dir"
        local socket_path="$socket_dir/python-sidecar.sock"
    fi

    echo "" >> .env
    echo "# Python Sidecar (Unix socket)" >> .env
    echo "PYTHON_SIDECAR_SOCKET=$socket_path" >> .env
    log_success "Added Unix socket config to .env"
    log_info "Socket path: $socket_path"
else
    log_success "Unix socket config already in .env"
fi

if ! grep -q "VITE_RUST_BACKEND_URL" .env 2>/dev/null; then
    echo "" >> .env
    echo "# Frontend → Rust backend" >> .env
    echo "VITE_RUST_BACKEND_URL=http://localhost:8001" >> .env
    log_success "Added Rust backend URL to .env"
else
    log_success "Rust backend URL already in .env"
fi

if ! grep -q "ASTRO_PORT" .env 2>/dev/null; then
    echo "" >> .env
    echo "# Astro dev server port" >> .env
    echo "ASTRO_PORT=4321" >> .env
    log_success "Added Astro port to .env"
else
    log_success "Astro port already in .env"
fi

if ! grep -q "PUBLIC_API_URL" .env 2>/dev/null; then
    echo "" >> .env
    echo "# Public API URL for frontend" >> .env
    echo "PUBLIC_API_URL=http://localhost:8001" >> .env
    log_success "Added public API URL to .env"
else
    log_success "Public API URL already in .env"
fi

log_success "Environment setup completed"
exit 0