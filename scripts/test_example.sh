#!/bin/bash
# FullStackHex Test Example
# Demonstrates the test framework; exercises common.sh utility functions.
#
# Usage:
#   bash scripts/test_example.sh

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/test/helpers.sh"

# ── Tests ────────────────────────────────────────────────────────────────────

# confirm_action should return 0 immediately in dry-run mode
test_confirm_action_dry_run() {
    test_setup
    local result
    result=0
    confirm_action "Test prompt" || result=$?
    test_teardown
    assert_exit_code 0 "$result" "confirm_action should succeed in dry-run mode"
}

# safe_remove should not delete the file when DRY_RUN=true
test_safe_remove_dry_run() {
    test_setup
    local tmp_file
    tmp_file="$MOCK_FILE_DIR/test_file.txt"
    echo "content" > "$tmp_file"
    safe_remove "$tmp_file"
    local result
    result=0
    assert_file_exists "$tmp_file" "safe_remove in dry-run should not delete file" || result=1
    test_teardown
    return $result
}

# assert_equals passes when values match
test_assert_equals_pass() {
    assert_equals "hello" "hello" "equal strings should pass"
}

# assert_equals should return non-zero when values differ
test_assert_equals_mismatch() {
    if assert_equals "hello" "world" "mismatch check" 2>/dev/null; then
        log_error "[FAIL] assert_equals should have returned non-zero on mismatch"
        return 1
    fi
    log_success "[PASS] assert_equals correctly returned non-zero on mismatch"
    return 0
}

# assert_contains finds a substring
test_assert_contains() {
    assert_contains "hello world" "world" "should find substring 'world'"
}

# assert_not_contains should pass when the needle is absent
test_assert_not_contains() {
    assert_not_contains "hello world" "foobar" "should not find 'foobar'"
}

# mock_command should log a [MOCK] message and not execute the real command
test_mock_command() {
    test_setup
    local output
    output=$(mock_command false 2>&1)
    local result
    result=0
    assert_contains "$output" "[MOCK]" "mock_command should emit [MOCK] prefix" || result=1
    test_teardown
    return $result
}

# mock_network_calls should return the pre-registered response in test mode
test_mock_network_calls() {
    test_setup
    export MOCK_HTTP_RESPONSES="http://example.com/health=OK"
    local response
    response=$(mock_network_calls "http://example.com/health" 2>/dev/null)
    local result
    result=0
    assert_equals "OK" "$response" "mock_network_calls should return registered response" || result=1
    test_teardown
    return $result
}

# mock_read_file should read from MOCK_FILE_DIR in test mode
test_mock_read_file() {
    test_setup
    echo "mock content" > "$MOCK_FILE_DIR/myfile.txt"
    local content
    content=$(mock_read_file "/real/path/myfile.txt" 2>/dev/null)
    local result
    result=0
    assert_equals "mock content" "$content" "mock_read_file should return mock content" || result=1
    test_teardown
    return $result
}

# mock_write_file should write to MOCK_FILE_DIR in test mode
test_mock_write_file() {
    test_setup
    mock_write_file "/real/path/output.txt" "written content" 2>/dev/null
    local result
    result=0
    assert_file_exists "$MOCK_FILE_DIR/output.txt" "mock_write_file should create file in mock dir" || result=1
    test_teardown
    return $result
}

# assert_file_exists should fail when file is absent
test_assert_file_exists_missing() {
    if assert_file_exists "/nonexistent/path/file.txt" "should not exist" 2>/dev/null; then
        log_error "[FAIL] assert_file_exists should have returned non-zero for missing file"
        return 1
    fi
    log_success "[PASS] assert_file_exists correctly returned non-zero for missing file"
    return 0
}

# assert_command_exists should pass for a known command and fail for a bogus one
test_assert_command_exists() {
    local result
    result=0
    assert_command_exists "bash" "bash should exist" || result=1
    if assert_command_exists "_nonexistent_cmd_xyz_" "should not exist" 2>/dev/null; then
        log_error "[FAIL] assert_command_exists should have returned non-zero for missing command"
        result=1
    fi
    return $result
}

# test_summary should return 1 when there are failures
test_summary_with_failure() {
    # Temporarily save and reset counters
    local saved_pass
    saved_pass=$_TEST_PASS
    local saved_fail
    saved_fail=$_TEST_FAIL
    _TEST_PASS=2
    _TEST_FAIL=1
    local exit_code
    exit_code=0
    test_summary 2>/dev/null || exit_code=$?
    # Restore counters
    _TEST_PASS=$saved_pass
    _TEST_FAIL=$saved_fail
    assert_exit_code 1 "$exit_code" "test_summary should return 1 when there are failures"
}

# ── Run all tests ─────────────────────────────────────────────────────────────
run_test "confirm_action in dry-run mode"      test_confirm_action_dry_run
run_test "safe_remove in dry-run mode"         test_safe_remove_dry_run
run_test "assert_equals pass case"             test_assert_equals_pass
run_test "assert_equals mismatch case"         test_assert_equals_mismatch
run_test "assert_contains"                     test_assert_contains
run_test "assert_not_contains"                 test_assert_not_contains
run_test "mock_command in test mode"           test_mock_command
run_test "mock_network_calls in test mode"     test_mock_network_calls
run_test "mock_read_file in test mode"         test_mock_read_file
run_test "mock_write_file in test mode"        test_mock_write_file
run_test "assert_file_exists missing file"     test_assert_file_exists_missing
run_test "assert_command_exists"               test_assert_command_exists
run_test "test_summary returns 1 on failures"  test_summary_with_failure

test_summary
