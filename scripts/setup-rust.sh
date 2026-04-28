#!/bin/bash
# FullStackHex Rust Workspace Setup
# Creates and configures the Rust workspace and crates

# Parse command-line arguments
DRY_RUN=false
SKIP_BUILD=false

while [[ "$#" -gt 0 ]]; do
    case "$1" in
        --dry-run)
            DRY_RUN=true
            shift
            ;;
        --skip-build)
            SKIP_BUILD=true
            shift
            ;;
        -h|--help)
            echo "Usage: $0 [--dry-run] [--skip-build]"
            echo ""
            echo "Options:"
            echo "  --dry-run      Show what would be done without doing it"
            echo "  --skip-build  Skip building the workspace"
            echo "  -h, --help   Show this help message"
            exit 0
            ;;
        *)
            shift
            ;;
    esac
done

export DRY_RUN

# Source common functions and configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"
source "$SCRIPT_DIR/config.sh"

log_info "Creating Rust workspace..."

if [ "$DRY_RUN" = true ]; then
    log_warning "DRY-RUN mode: no changes will be made"
fi

# Create backend directory if it doesn't exist
mkdir -p backend
pushd backend > /dev/null

# Create workspace Cargo.toml if not exists
if [ ! -f Cargo.toml ]; then
    log_info "Creating workspace Cargo.toml..."
    cat > Cargo.toml << 'EOF'
[workspace]
members = ["crates/*"]
resolver = "3"

[workspace.package]
description = "FullStackHex project"
license = "MIT"
repository = "https://github.com/yourusername/yourrepo"
authors = ["Your Name <your@email.com>"]

[workspace.dependencies]
tokio = { version = "1", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
axum = "0.8"
sqlx = { version = "0.8", features = ["postgres", "runtime-tokio-native-tls"] }
tower = "0.5"
tower-http = "0.5"
serde_json = "1.0"

[profile.release]
lto = true
EOF
else
    log_success "Workspace Cargo.toml already exists"
fi

# Create migration directory for sqlx
mkdir -p crates/db/migrations

# Create individual crates if they don't exist or are invalid
for crate in api core db python-sidecar; do
    local crate_valid=false
    if [ -d "crates/$crate" ] && [ -f "crates/$crate/Cargo.toml" ]; then
        crate_valid=true
    fi

    if [ "$crate_valid" = true ]; then
        log_success "Crate already exists: $crate"
    else
        if [ -d "crates/$crate" ]; then
            log_warning "Removing invalid crate directory: $crate..."
            rm -rf "crates/$crate"
        fi
        log_info "Creating crate: $crate..."
        cargo new --lib --edition 2024 "crates/$crate"
        # Overwrite cargo new's minimal Cargo.toml with workspace-aware version + dev-deps
        case "$crate" in
            api)
                cat > "crates/$crate/Cargo.toml" << 'CARGO_EOF'
[package]
name = "api"
version = "0.1.0"
edition = "2024"
description.workspace = true
license.workspace = true
repository.workspace = true
authors.workspace = true

[dependencies]

[dev-dependencies]
tokio = { workspace = true }
axum = { workspace = true }
tower = { workspace = true }
serde_json = { workspace = true }
CARGO_EOF
                ;;
            *)
                cat > "crates/$crate/Cargo.toml" << CARGO_EOF
[package]
name = "$crate"
version = "0.1.0"
edition = "2024"
description.workspace = true
license.workspace = true
repository.workspace = true
authors.workspace = true

[dependencies]

[dev-dependencies]
serde_json = { workspace = true }
CARGO_EOF
                ;;
        esac
    fi
done

# Build workspace
log_info "Building workspace..."
cargo build --workspace
log_success "Rust workspace ready"

popd > /dev/null
log_success "Rust workspace setup completed"
exit 0