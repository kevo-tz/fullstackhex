#!/bin/bash
# FullStackHex Modular Installer
# Main orchestrator for the installation process

# Source common functions and configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"
source "$SCRIPT_DIR/config.sh"

# Default values
SKIP_PYTHON=false

# Parse command-line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --skip-python)
            SKIP_PYTHON=true
            shift
            ;;
        --help|-h)
            echo "Usage: $0 [options]"
            echo ""
            echo "Options:"
            echo "  --skip-python    Skip Python check and scaffolding"
            echo "  --help, -h       Show this help message"
            exit 0
            ;;
        *)
            log_error "Unknown argument: $1"
            echo "Usage: $0 [--skip-python]"
            exit 1
            ;;
    esac
done

# Ensure we're in the repository root
if [ "$PWD" != "$REPO_ROOT" ]; then
    log_warning "Switching to repository root: $REPO_ROOT"
    cd "$REPO_ROOT"
fi

log_info "FullStackHex - Full Initialization"
echo ""

# Validate repository structure
if [ ! -f "$REPO_ROOT/scripts/install.sh" ] || [ ! -d "$REPO_ROOT/compose" ]; then
    log_error "Could not resolve repository root from script location."
    log_error "Expected to find scripts/install.sh and compose/ under: $REPO_ROOT"
    exit 1
fi

# Check for nested frontend (warning only)
if [ -d "$REPO_ROOT/backend/frontend" ]; then
    log_warning "Found nested frontend at backend/frontend (likely accidental duplicate)."
    log_warning "Canonical frontend path is: $REPO_ROOT/frontend"
fi

# Run installation steps
log_info "Starting installation process..."

# Step 1: Check and install dependencies
"$SCRIPT_DIR/install-deps.sh" --skip-python="$SKIP_PYTHON"
DEPS_RESULT=$?

if [ $DEPS_RESULT -ne 0 ]; then
    log_error "Dependency installation failed. Exiting."
    exit $DEPS_RESULT
fi

# Step 2: Setup environment
"$SCRIPT_DIR/setup-env.sh"
ENV_RESULT=$?

if [ $ENV_RESULT -ne 0 ]; then
    log_error "Environment setup failed. Exiting."
    exit $ENV_RESULT
fi

# Step 3: Create Rust workspace
"$SCRIPT_DIR/setup-rust.sh"
RUST_RESULT=$?

if [ $RUST_RESULT -ne 0 ]; then
    log_error "Rust workspace setup failed. Exiting."
    exit $RUST_RESULT
fi

# Step 4: Setup Python sidecar (if not skipped)
if [ "$SKIP_PYTHON" = false ]; then
    "$SCRIPT_DIR/setup-python.sh"
    PYTHON_RESULT=$?

    if [ $PYTHON_RESULT -ne 0 ]; then
        log_error "Python sidecar setup failed. Exiting."
        exit $PYTHON_RESULT
    fi
else
    log_warning "Python check and scaffolding skipped (--skip-python)"
fi

# Step 5: Setup frontend
"$SCRIPT_DIR/setup-frontend.sh"
FRONTEND_RESULT=$?

if [ $FRONTEND_RESULT -ne 0 ]; then
    log_error "Frontend setup failed. Exiting."
    exit $FRONTEND_RESULT
fi

# Step 6: Generate test suites
"$SCRIPT_DIR/setup-tests.sh"
TESTS_RESULT=$?

if [ $TESTS_RESULT -ne 0 ]; then
    log_error "Test suite generation failed. Exiting."
    exit $TESTS_RESULT
fi

log_success "FullStackHex initialization complete!"
echo ""
echo "Architecture: Rust-centric with Python sidecar (Unix socket)"
echo ""
echo "Next steps:"
echo "  1. docker compose -f compose/dev.yml up -d"
echo "  2. cd backend && cargo run -p api"
echo "     (starts Axum on port 8001)"
echo "  3. cd frontend && bun run dev"
echo ""
echo "Verify versions:"
echo "  rustc --version    (should show latest stable)"
echo "  bun --version     (should show latest)"
echo "  uv --version      (should show latest)"
echo ""

exit 0