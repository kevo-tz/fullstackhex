#!/bin/bash
# FullStackHex Test Framework - Helper Functions
# Source this file in test scripts to get test lifecycle and assertion utilities
#
# Usage:
#   source "$(dirname "${BASH_SOURCE[0]}")/../test/helpers.sh"
#   test_setup; ...; test_teardown
#   run_test "name" my_test_function
#   test_summary

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../common.sh"

# ── Test state ──────────────────────────────────────────────────────────────
_TEST_PASS=0
_TEST_FAIL=0
_TEST_CURRENT=""

# ── Lifecycle ────────────────────────────────────────────────────────────────

# Enable test/dry-run mode and create an isolated temp directory for file mocks
test_setup() {
    export TEST_MODE=true
    export DRY_RUN=true
    export MOCK_FILE_DIR
    MOCK_FILE_DIR=$(mktemp -d)
    log_info "[TEST] Setup complete. MOCK_FILE_DIR=$MOCK_FILE_DIR"
}

# Disable test/dry-run mode and remove the temp directory
test_teardown() {
    if [ -n "$MOCK_FILE_DIR" ] && [ -d "$MOCK_FILE_DIR" ]; then
        rm -rf "$MOCK_FILE_DIR"
    fi
    unset TEST_MODE DRY_RUN MOCK_FILE_DIR MOCK_HTTP_RESPONSES
    log_info "[TEST] Teardown complete"
}

# ── Runner ───────────────────────────────────────────────────────────────────

# Run a single named test function and record pass/fail
run_test() {
    local name="$1"
    local fn="$2"
    _TEST_CURRENT="$name"
    if "$fn"; then
        log_success "[TEST PASS] $name"
        _TEST_PASS=$(( _TEST_PASS + 1 ))
    else
        log_error "[TEST FAIL] $name"
        _TEST_FAIL=$(( _TEST_FAIL + 1 ))
    fi
}

# Print summary and exit with 0 (all passed) or 1 (some failed)
test_summary() {
    echo "" >&2
    log_info "Test results: $_TEST_PASS passed, $_TEST_FAIL failed"
    if [ "$_TEST_FAIL" -eq 0 ]; then
        log_success "All tests passed"
        return 0
    else
        log_error "$_TEST_FAIL test(s) failed"
        return 1
    fi
}

# ── Extra assertions ─────────────────────────────────────────────────────────

# Assert a string does NOT contain a substring
assert_not_contains() {
    local haystack="$1"
    local needle="$2"
    local message="${3:-Assertion failed}"

    if ! echo "$haystack" | grep -q "$needle"; then
        log_success "[PASS] $message"
        return 0
    else
        log_error "[FAIL] $message"
        log_error "  Needle found (should not be): '$needle'"
        return 1
    fi
}

# Assert two exit codes match
assert_exit_code() {
    local expected_code="$1"
    local actual_code="$2"
    local message="${3:-Exit code assertion failed}"

    if [ "$expected_code" = "$actual_code" ]; then
        log_success "[PASS] $message (exit code: $actual_code)"
        return 0
    else
        log_error "[FAIL] $message"
        log_error "  Expected exit code: $expected_code"
        log_error "  Actual exit code:   $actual_code"
        return 1
    fi
}
