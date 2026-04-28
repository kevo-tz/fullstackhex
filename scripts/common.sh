#!/bin/bash
# FullStackHex Common Functions Library
# Shared utility functions for all scripts

# Add Go bin to PATH if not already present
if [[ ":$PATH:" != *":$HOME/go/bin:"* ]]; then
    export PATH="$PATH:$HOME/go/bin"
fi

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Logging functions
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Source .env file if it exists
load_env() {
    if [ -f .env ]; then
        set -a
        source .env
        set +a
        log_success "Loaded environment from .env"
    else
        log_warning ".env file not found"
    fi
}

# Check if a command exists
command_exists() {
    command -v "$1" &> /dev/null
}

# Check service health with timeout
check_service_http() {
    local name="$1"
    local url="$2"
    local timeout="${3:-30}"
    local verbose="${4:-false}"
    
    local start_time=$(date +%s)
    local end_time=$((start_time + timeout))
    
    if [ "$verbose" = true ]; then
        log_info "Checking $name at $url..."
    fi
    
    while [ $(date +%s) -lt $end_time ]; do
        if curl --silent --fail "$url" > /dev/null 2>&1; then
            log_success "$name is healthy"
            return 0
        fi
        sleep 1
    done
    
    log_error "$name is not responding"
    return 1
}

# Get repository root
get_repo_root() {
    local script_dir
    script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    echo "$(cd "$script_dir/.." && pwd)"
}

# Validate required environment variables
validate_env_vars() {
    local required_vars=("$@")
    local missing=0
    
    for var in "${required_vars[@]}"; do
        if [ -z "${!var}" ]; then
            log_error "Environment variable $var is not set"
            missing=$((missing + 1))
        fi
    done
    
    return $missing
}

# Create directory if it doesn't exist
ensure_dir() {
    local dir="$1"
    if [ ! -d "$dir" ]; then
        mkdir -p "$dir"
        log_success "Created directory: $dir"
    fi
}

# Get current timestamp
get_timestamp() {
    date -Iseconds 2>/dev/null || date +"%Y-%m-%dT%H:%M:%S%z"
}

# Get git commit hash
get_git_commit() {
    git rev-parse HEAD 2>/dev/null || echo "unknown"
}