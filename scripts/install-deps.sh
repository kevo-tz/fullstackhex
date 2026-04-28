#!/bin/bash
# FullStackHex Dependency Installer
# Installs required development tools and dependencies

# Source common functions and configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"
source "$SCRIPT_DIR/config.sh"

# Default values
SKIP_PYTHON=false

# Parse command-line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --skip-python*)
            if [[ "$1" == *"="* ]]; then
                SKIP_PYTHON="${1#*=}"
            else
                SKIP_PYTHON="$2"
                shift
            fi
            ;;
        --help|-h)
            echo "Usage: $0 [--skip-python=true|false]"
            echo ""
            echo "Options:"
            echo "  --skip-python    Skip Python check and uv installation (default: false)"
            echo "  --help, -h       Show this help message"
            exit 0
            ;;
        *)
            log_warning "Unknown argument: $1 (ignoring)"
            ;;
    esac
    shift
done

log_info "Checking and installing dependencies..."

# Check and install Rust
install_rust() {
    if command -v rustc &> /dev/null; then
        local version=$(rustc --version)
        log_success "Rust already installed: $version"
    else
        log_info "Installing Rust..."
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        source "$HOME/.cargo/env"
        log_success "Rust installed: $(rustc --version)"
    fi
    rustup update stable
}

# Check and install Bun
install_bun() {
    if command -v bun &> /dev/null; then
        local version=$(bun --version)
        log_success "Bun already installed: v$version"
        bun upgrade
        return 0
    fi

    log_info "Installing Bun..."
    curl -fsSL https://bun.sh/install | bash

    # Ensure bun bin is on PATH for current session
    if [ -d "$HOME/.bun/bin" ]; then
        export PATH="$HOME/.bun/bin:$PATH"
    fi

    # Detect active shell and its rc file
    local shell_name
    shell_name=$(basename "${SHELL:-/bin/sh}")
    local rc_file=""
    case "$shell_name" in
        bash)
            rc_file="$HOME/.bashrc"
            ;;
        zsh)
            rc_file="$HOME/.zshrc"
            ;;
        fish)
            rc_file="$HOME/.config/fish/config.fish"
            ;;
        *)
            # Fallback to .profile for sh, dash, etc.
            rc_file="$HOME/.profile"
            ;;
    esac

    # Ensure PATH is persisted in the rc file
    if [ -n "$rc_file" ]; then
        # Check if already configured
        if [ -f "$rc_file" ] && grep -q 'bun/bin' "$rc_file" 2>/dev/null; then
            log_success "Bun PATH already configured in $rc_file"
        else
            mkdir -p "$(dirname "$rc_file")" 2>/dev/null || true
            echo '' >> "$rc_file"
            echo '# Added by FullStackHex install.sh' >> "$rc_file"
            if [[ "$shell_name" = "fish" ]]; then
                echo 'set -gx PATH "$HOME/.bun/bin" $PATH' >> "$rc_file"
            else
                echo 'export PATH="$HOME/.bun/bin:$PATH"' >> "$rc_file"
            fi
            log_success "Added Bun to PATH in $rc_file"
        fi

        # Source the rc file for the current session (best-effort)
        # shellcheck disable=SC1090
        source "$rc_file" 2>/dev/null || true
    fi

    # Verify bun is now accessible
    if command -v bun &> /dev/null; then
        log_success "Bun installed: v$(bun --version)"
        bun upgrade
    else
        log_warning "Bun installed but not on PATH. Run: source $rc_file"
        log_warning "  Or restart your shell."
    fi
}

# Check Python (don't auto-install, just check)
check_python() {
    if [ "$SKIP_PYTHON" = true ]; then
        log_warning "Skipping Python check (--skip-python set)"
        return 0
    fi

    if command -v python3 &> /dev/null; then
        local version=$(python3 --version 2>&1)
        local major=$(python3 -c 'import sys; print(sys.version_info.major)')
        local minor=$(python3 -c 'import sys; print(sys.version_info.minor)')

        # Accept Python >= 3.14, including future major versions (e.g., 4.x).
        if (( major > 3 )) || (( major == 3 && minor >= 14 )); then
            log_success "Python already installed: $version"
        else
            log_error "Python 3.14+ required. Found: $version"
            log_warning "  Install with: pyenv install 3.14 or your package manager"
            return 1
        fi
    else
        log_error "Python 3 not found"
        log_warning "  Install with: pyenv install 3.14 or your package manager"
        return 1
    fi
}

# Check Docker (don't auto-install)
check_docker() {
    if command -v docker &> /dev/null; then
        local version=$(docker --version)
        log_success "Docker already installed: $version"
    else
        log_error "Docker not found - please install manually"
        log_warning "  Visit: https://docs.docker.com/get-docker/"
        return 1
    fi

    if command -v docker-compose &> /dev/null || docker compose version &> /dev/null; then
        log_success "Docker Compose available"
    else
        log_error "Docker Compose not found"
        return 1
    fi
}

# Check and install uv (Python package manager)
install_uv() {
    if command -v uv &> /dev/null; then
        local version=$(uv --version)
        log_success "uv already installed: $version"
    else
        log_info "Installing uv (Python package manager)..."
        curl -LsSf https://astral.sh/uv/install.sh | sh
        
        # uv installs to $HOME/.local/bin by default
        # Also check cargo/bin as fallback
        if [ -x "$HOME/.local/bin/uv" ]; then
            export PATH="$HOME/.local/bin:$PATH"
        elif [ -x "$HOME/.cargo/bin/uv" ]; then
            export PATH="$HOME/.cargo/bin:$PATH"
        fi
        log_success "uv installed: $(uv --version)"
    fi
}

# Main installation process
install_rust
install_bun

if [ "$SKIP_PYTHON" != true ]; then
    check_python
    if [ $? -eq 0 ]; then
        install_uv
    fi
fi

check_docker

log_success "Dependency installation completed"
exit 0