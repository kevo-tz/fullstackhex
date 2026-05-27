#!/usr/bin/env bash
set -euo pipefail

# FullStackHex lint/format/typecheck — mirrors CI steps exactly.
# Run before pushing to catch failures that CI would flag.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/config.sh"

REPO_ROOT="$(get_repo_root)"
cd "$REPO_ROOT" || exit

EXIT=0
PASS=0
SKIP=0

check() {
    local label="$1"
    shift
    echo "--- $label ---"
    if "$@"; then
        echo "✓ $label passed"
        PASS=$((PASS + 1))
    else
        echo "✗ $label FAILED"
        EXIT=1
    fi
    echo ""
}

warn_skip() {
    local label="$1"
    local cmd="$2"
    echo "--- $label ---"
    echo "SKIP: $cmd not found — install it to match CI"
    SKIP=$((SKIP + 1))
    echo ""
}

# --- Rust (CI working-directory: backend) ---
if command -v cargo &>/dev/null; then
    check "cargo fmt --check"        bash -c 'cd backend && cargo fmt --all -- --check'
    check "cargo clippy"             bash -c 'cd backend && cargo clippy --locked --all-targets --all-features -- -D warnings'
else
    warn_skip "cargo fmt --check" "cargo"
    warn_skip "cargo clippy" "cargo"
fi

# --- Python (CI working-directory: py-api) ---
if command -v uv &>/dev/null; then
    check "ruff check"               bash -c 'cd py-api && uv run ruff check .'
    check "ruff format --check"      bash -c 'cd py-api && uv run ruff format --check .'
else
    warn_skip "ruff check" "uv"
    warn_skip "ruff format --check" "uv"
fi

# --- Frontend (CI working-directory: frontend) ---
if command -v bun &>/dev/null; then
    check "eslint"                   bash -c 'cd frontend && bun run lint'
    check "astro check"              bash -c 'cd frontend && bun run typecheck'
else
    warn_skip "eslint" "bun"
    warn_skip "astro check" "bun"
fi

# --- Shell ---
if command -v shellcheck &>/dev/null; then
    check "shellcheck"               shellcheck -x --source-path=scripts/ scripts/*.sh
else
    warn_skip "shellcheck" "shellcheck"
fi

# --- Summary ---
echo "=================================="
echo "Passed: $PASS | Skipped: $SKIP | Failed: $([ "$EXIT" -ne 0 ] && echo "yes" || echo "none")"
if [ "$SKIP" -gt 0 ]; then
    echo "Install missing tools to get full CI coverage before pushing."
fi

exit $EXIT
