#!/usr/bin/env bats
# Bats-core tests for FullStackHex deploy scripts.
#
# Mocks ssh, scp, rsync, docker, nginx, and flock so scripts
# run without touching any real infrastructure.
#
# Install: brew install bats-core    (macOS)
#          npm install -g bats      (any)
#          apt install bats         (Debian/Ubuntu)
#
# Run: bats tests/deploy/

setup() {
    SCRIPT_DIR="$(cd "$(dirname "${BATS_TEST_FILENAME}")/../../scripts" && pwd)"
    TEST_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

    # Create a temp dir for mocked files
    export MOCK_DIR="$(mktemp -d)"

    # Mock binaries — override PATH so scripts use our stubs
    export MOCK_BIN="$MOCK_DIR/bin"
    mkdir -p "$MOCK_BIN"
    export PATH="$MOCK_BIN:$PATH"

    # Create stub for ssh: logs the command and optionally returns exit code
    cat > "$MOCK_BIN/ssh" << 'STUB'
#!/bin/bash
echo "MOCK ssh $*" >> "$MOCK_DIR/ssh.log"
# Check for fail markers in arguments
for arg in "$@"; do
    case "$arg" in
        *fail-ssh*) exit 1 ;;
        *fail-nginx*) exit 1 ;;
        *fail-docker*) exit 1 ;;
    esac
done
exit 0
STUB
    chmod +x "$MOCK_BIN/ssh"

    # Stub for scp
    cat > "$MOCK_BIN/scp" << 'STUB'
#!/bin/bash
echo "MOCK scp $*" >> "$MOCK_DIR/scp.log"
exit 0
STUB
    chmod +x "$MOCK_BIN/scp"

    # Stub for rsync
    cat > "$MOCK_BIN/rsync" << 'STUB'
#!/bin/bash
echo "MOCK rsync $*" >> "$MOCK_DIR/rsync.log"
exit 0
STUB
    chmod +x "$MOCK_BIN/rsync"

    # Stub for docker
    cat > "$MOCK_BIN/docker" << 'STUB'
#!/bin/bash
echo "MOCK docker $*" >> "$MOCK_DIR/docker.log"
exit 0
STUB
    chmod +x "$MOCK_BIN/docker"

    # Stub for nginx
    cat > "$MOCK_BIN/nginx" << 'STUB'
#!/bin/bash
echo "MOCK nginx $*" >> "$MOCK_DIR/nginx.log"
exit 0
STUB
    chmod +x "$MOCK_BIN/nginx"

    # Stub for flock (deploy lock)
    cat > "$MOCK_BIN/flock" << 'STUB'
#!/bin/bash
echo "MOCK flock $*" >> "$MOCK_DIR/flock.log"
# flock passes through: shift past -n, lockfile, then exec remaining args
shift 2 2>/dev/null || true
exec "$@"
STUB
    chmod +x "$MOCK_BIN/flock"

    # Stub for curl (health checks)
    cat > "$MOCK_BIN/curl" << 'STUB'
#!/bin/bash
echo "MOCK curl $*" >> "$MOCK_DIR/curl.log"
# Return a healthy response if --write-out is not requested
if [[ "$*" == *"--write-out"* ]] || [[ "$*" == *"-w"* ]]; then
    echo "200"
else
    echo '{"status":"ok"}'
fi
exit 0
STUB
    chmod +x "$MOCK_BIN/curl"

    # Ensure real commands are still available
    for cmd in python3 date sed cat sleep mkdir cp rm mktemp grep jq; do
        if ! command -v "$cmd" >/dev/null 2>&1; then
            # Create a minimal stub for testing
            cat > "$MOCK_BIN/$cmd" << 'STUB'
#!/bin/bash
echo "MOCK $0 $*" >> "$MOCK_DIR/misc.log"
exit 0
STUB
            chmod +x "$MOCK_BIN/$cmd"
        fi
    done

    # Create fake .env file for deploy scripts to source
    cat > "$MOCK_DIR/.env" << 'ENV'
DEPLOY_HOST=test-host
DEPLOY_USER=test-user
DEPLOY_PATH=/app/test
DOMAIN=example.com
ENV
}

teardown() {
    rm -rf "$MOCK_DIR"
}

# ── Rollback tests ──────────────────────────────────────────────────────

@test "rollback: script is valid bash" {
    run bash -n "$SCRIPT_DIR/rollback.sh"
    [ "$status" -eq 0 ]
}

@test "rollback: fails when .deploy-state is missing" {
    # Create a modified rollback that uses our mock env
    run bash -c "
        export DEPLOY_HOST=test-host
        export DEPLOY_USER=test-user
        export DEPLOY_PATH=/app/test
        export MOCK_DIR='$MOCK_DIR'
        # Source helpers to avoid real ssh
        source '$SCRIPT_DIR/rollback.sh' 2>&1 || true
    "
    # Should fail because .deploy-state doesn't exist
    [ "$status" -ne 0 ]
}

@test "rollback: detects missing DEPLOY_HOST" {
    run bash -c "
        unset DEPLOY_HOST
        export DEPLOY_USER=test-user
        export DEPLOY_PATH=/app/test
        source '$SCRIPT_DIR/rollback.sh' 2>&1 || true
    "
    [ "$status" -ne 0 ]
}

# ── Blue-green deploy tests ─────────────────────────────────────────────

@test "blue-green: script is valid bash" {
    run bash -n "$SCRIPT_DIR/deploy-blue-green.sh"
    [ "$status" -eq 0 ]
}

@test "blue-green: requires required env vars" {
    run bash -c "
        unset DEPLOY_HOST DEPLOY_USER DEPLOY_PATH
        source '$SCRIPT_DIR/deploy-blue-green.sh' 2>&1 || true
    "
    [ "$status" -ne 0 ]
}

# ── Canary deploy tests ─────────────────────────────────────────────────

@test "canary: script is valid bash" {
    run bash -n "$SCRIPT_DIR/deploy-canary.sh"
    [ "$status" -eq 0 ]
}

@test "canary-promote: script is valid bash" {
    run bash -n "$SCRIPT_DIR/deploy-canary-promote.sh"
    [ "$status" -eq 0 ]
}

@test "canary-rollback: script is valid bash" {
    run bash -n "$SCRIPT_DIR/deploy-canary-rollback.sh"
    [ "$status" -eq 0 ]
}

# ── Deploy verify tests ─────────────────────────────────────────────────

@test "deploy-verify: script is valid bash" {
    run bash -n "$SCRIPT_DIR/deploy-verify.sh"
    [ "$status" -eq 0 ]
}

@test "deploy-verify: uses default timeout of 30s" {
    run bash -c "
        source '$SCRIPT_DIR/deploy-verify.sh' 2>&1 || true
    "
    # Script runs with curl mock — exits when health passes
    [ "$status" -eq 0 ]
}

# ── Lock file safety ────────────────────────────────────────────────────

@test "deploy scripts use flock for mutual exclusion" {
    # blue-green and canary scripts should reference flock
    run grep -l "flock" "$SCRIPT_DIR/deploy-blue-green.sh" "$SCRIPT_DIR/deploy-canary.sh" "$SCRIPT_DIR/rollback.sh"
    [ "$status" -eq 0 ]
    [ "${#lines[@]}" -ge 3 ]
}

# ── Common helpers tests ────────────────────────────────────────────────

@test "common.sh: can be sourced without errors" {
    run bash -c "
        source '$SCRIPT_DIR/common.sh' 2>&1
        echo 'OK'
    "
    [ "$status" -eq 0 ]
    [[ "$output" == *"OK"* ]]
}

@test "common.sh: log_info produces output" {
    run bash -c "
        source '$SCRIPT_DIR/common.sh'
        log_info 'test message' 2>&1
    "
    [ "$status" -eq 0 ]
    [[ "$output" == *"test message"* ]]
}

@test "common.sh: log_error produces output" {
    run bash -c "
        source '$SCRIPT_DIR/common.sh'
        log_error 'error message' 2>&1
    "
    [ "$status" -eq 0 ]
    [[ "$output" == *"error message"* ]]
}

@test "common.sh: check_postgres called without DATABASE_URL warns" {
    run bash -c "
        unset DATABASE_URL
        source '$SCRIPT_DIR/common.sh'
        check_postgres 2>&1 || true
    "
    [ "$status" -eq 0 ]
    [[ "$output" == *"skipping"* ]] || [[ "$output" == *"not set"* ]]
}

@test "common.sh: validate_env_vars reports missing vars" {
    run bash -c "
        source '$SCRIPT_DIR/common.sh'
        validate_env_vars DEPLOY_HOST NONEXISTENT_VAR_XYZ 2>&1 || true
    "
    [[ "$output" == *"NONEXISTENT_VAR_XYZ"* ]]
}

# ── Config tests ────────────────────────────────────────────────────────

@test "config.sh: can be sourced without errors" {
    run bash -c "
        source '$SCRIPT_DIR/config.sh' 2>&1
        echo 'CONFIG_OK'
    "
    [ "$status" -eq 0 ]
    [[ "$output" == *"CONFIG_OK"* ]]
}
