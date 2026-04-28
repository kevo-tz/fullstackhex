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

# Dry-run mode support
DRY_RUN="${DRY_RUN:-false}"

dry_run_mode() {
    [ "$DRY_RUN" = "true" ]
}

log_dry_run() {
    if dry_run_mode; then
        log_warning "[DRY-RUN] $1"
    fi
}

# Safety: prompt for confirmation
confirm_action() {
    local prompt="${1:-Continue?}"
    local response
    
    if dry_run_mode; then
        log_warning "[DRY-RUN] Would confirm: $prompt"
        return 0
    fi
    
    if [ -n "$CI_NONINTERACTIVE" ]; then
        log_warning "Non-interactive mode, skipping: $prompt"
        return 1
    fi
    
    echo -n "$prompt [y/N] "
    read -r response
    
    case "$response" in
        y|Y) return 0 ;;
        *) return 1 ;;
    esac
}

# Safety: safe remove with backup
safe_remove() {
    local path="$1"
    local backup_dir="${2:-.backup}"
    
    if [ -e "$path" ]; then
        log_dry_run "Would remove: $path"
        
        if ! dry_run_mode; then
            if [ -d "$backup_dir" ] || mkdir -p "$backup_dir"; then
                local backup_name
                backup_name="$(basename "$path")_$(date +%Y%m%d_%H%M%S)"
                log_info "Backing up to: $backup_dir/$backup_name"
                mv "$path" "$backup_dir/$backup_name"
            fi
        fi
    fi
    
    return 0
}

# Safety: safe copy with backup option
safe_copy() {
    local src="$1"
    local dest="$2"
    local backup="${3:-false}"
    
    if [ ! -e "$src" ]; then
        log_error "Source does not exist: $src"
        return 1
    fi
    
    log_dry_run "Would copy: $src -> $dest"
    
    if dry_run_mode; then
        return 0
    fi
    
    if [ "$backup" = "true" ] && [ -e "$dest" ]; then
        local backup_name
        backup_name="$(basename "$dest")_$(date +%Y%m%d_%H%M%S)"
        log_info "Backing up existing: $dest -> .backup/$backup_name"
        mkdir -p .backup
        cp "$dest" ".backup/$backup_name"
    fi
    
    cp -r "$src" "$dest"
    log_success "Copied: $src -> $dest"
    return 0
}

# Safety: safe move
safe_move() {
    local src="$1"
    local dest="$2"
    
    if [ ! -e "$src" ]; then
        log_error "Source does not exist: $src"
        return 1
    fi
    
    log_dry_run "Would move: $src -> $dest"
    
    if dry_run_mode; then
        return 0
    fi
    
    mv "$src" "$dest"
    log_success "Moved: $src -> $dest"
    return 0
}

# Safety: check disk space
check_disk_space() {
    local required_kb="$1"
    local available_kb
    
    available_kb=$(df -k . | awk 'NR==2 {print $4}')
    
    if [ -n "$available_kb" ] && [ "$available_kb" -lt "$required_kb" ]; then
        log_error "Insufficient disk space. Required: ${required_kb}KB, Available: ${available_kb}KB"
        return 1
    fi
    
    log_success "Disk space check passed: ${available_kb}KB available"
    return 0
}

# Safety: check write permissions
check_write_permission() {
    local path="$1"
    
    if touch "$path.test" 2>/dev/null; then
        rm "$path.test"
        return 0
    fi
    
    log_error "No write permission for: $path"
    return 1
}