#!/bin/bash
# FullStackHex End-to-End Test
#
# Covers the full user journey:
#   start stack → register → login → /auth/me → upload file →
#   download file → delete file → verify health → cleanup
#
# Usage:
#   ./tests/e2e.sh                    # run against running services
#   ./tests/e2e.sh --start            # start infra + backend + frontend first
#   ./tests/e2e.sh --help             # show options
#
# Requires: curl, jq, docker compose (for --start)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

# ── Config ──────────────────────────────────────────────────────────────
BACKEND_URL="${BACKEND_URL:-http://localhost:8001}"
FRONTEND_URL="${FRONTEND_URL:-http://localhost:4321}"
TEST_EMAIL="e2e-shell-$(date +%s)@test.example.com"
TEST_PASSWORD="e2e-shell-test-password-123"
TOKEN=""
USER_ID=""

PASS=0
FAIL=0

# ── Helpers ─────────────────────────────────────────────────────────────

green() { printf "\033[32m%s\033[0m\n" "$*"; }
red()   { printf "\033[31m%s\033[0m\n" "$*"; }
bold()  { printf "\033[1m%s\033[0m\n" "$*"; }

pass() {
    green "  ✓ PASS: $1"
    PASS=$((PASS + 1))
}

fail() {
    red "  ✗ FAIL: $1"
    if [ -n "${2:-}" ]; then
        red "    $2"
    fi
    FAIL=$((FAIL + 1))
}

assert_status() {
    local expected="$1"
    local actual="$2"
    local msg="$3"
    if [ "$actual" -eq "$expected" ]; then
        pass "$msg"
    else
        fail "$msg (expected HTTP $expected, got $actual)"
    fi
}

assert_json_has() {
    local json="$1"
    local field="$2"
    local msg="$3"
    local val
    val=$(echo "$json" | jq -r ".$field // empty" 2>/dev/null)
    if [ -n "$val" ] && [ "$val" != "null" ]; then
        pass "$msg ($field=$val)"
    else
        fail "$msg (field '$field' missing or null)"
    fi
}

assert_contains() {
    local haystack="$1"
    local needle="$2"
    local msg="$3"
    if echo "$haystack" | grep -q "$needle"; then
        pass "$msg"
    else
        fail "$msg (expected to contain '$needle')"
    fi
}

# ── Step 1: Health check ────────────────────────────────────────────────

step_health() {
    bold "Step 1: Health check"

    local resp
    resp=$(curl -sk --max-time 5 "$BACKEND_URL/health" 2>/dev/null || echo '{"status":"error"}')
    local status
    status=$(echo "$resp" | jq -r '.status // "error"')
    if [ "$status" = "ok" ]; then
        pass "Backend /health returns ok"
    else
        fail "Backend /health — is the backend running on $BACKEND_URL?"
        echo "  Start: cd backend && cargo run -p api"
        exit 1
    fi

    resp=$(curl -sk --max-time 5 "$FRONTEND_URL/" 2>/dev/null || echo "")
    if echo "$resp" | grep -q "FullStackHex"; then
        pass "Frontend dashboard serves at $FRONTEND_URL"
    else
        fail "Frontend dashboard — is the frontend running on $FRONTEND_URL?"
        echo "  Start: cd frontend && bun run dev"
        exit 1
    fi
}

# ── Step 2: Register ────────────────────────────────────────────────────

step_register() {
    bold "Step 2: Register user"

    local resp http_status
    resp=$(curl -sk --max-time 10 \
        -X POST "$BACKEND_URL/auth/register" \
        -H "Content-Type: application/json" \
        -d "{\"email\":\"$TEST_EMAIL\",\"password\":\"$TEST_PASSWORD\",\"name\":\"E2E Shell Test\"}" \
        -w "\n%{http_code}" 2>/dev/null)

    http_status=$(echo "$resp" | tail -1)
    local body
    body=$(echo "$resp" | sed '$d')

    if [ "$http_status" = "404" ]; then
        fail "Registration — auth not configured (JWT_SECRET not set?)"
        exit 1
    fi

    assert_status 201 "$http_status" "POST /auth/register returns 201"

    TOKEN=$(echo "$body" | jq -r '.access_token // empty')
    USER_ID=$(echo "$body" | jq -r '.user.id // empty')

    assert_json_has "$body" "access_token" "Registration returns access_token"
    assert_json_has "$body" "user.id" "Registration returns user.id"
}

# ── Step 3: Login ───────────────────────────────────────────────────────

step_login() {
    bold "Step 3: Login"

    local resp http_status
    resp=$(curl -sk --max-time 10 \
        -X POST "$BACKEND_URL/auth/login" \
        -H "Content-Type: application/json" \
        -d "{\"email\":\"$TEST_EMAIL\",\"password\":\"$TEST_PASSWORD\"}" \
        -w "\n%{http_code}" 2>/dev/null)

    http_status=$(echo "$resp" | tail -1)
    local body
    body=$(echo "$resp" | sed '$d')

    assert_status 200 "$http_status" "POST /auth/login returns 200"
    assert_json_has "$body" "access_token" "Login returns access_token"

    TOKEN=$(echo "$body" | jq -r '.access_token // empty')
}

# ── Step 4: Wrong password login ────────────────────────────────────────

step_wrong_password() {
    bold "Step 4: Login with wrong password"

    local resp http_status
    resp=$(curl -sk --max-time 10 \
        -X POST "$BACKEND_URL/auth/login" \
        -H "Content-Type: application/json" \
        -d "{\"email\":\"$TEST_EMAIL\",\"password\":\"wrong-password\"}" \
        -w "\n%{http_code}" 2>/dev/null)

    http_status=$(echo "$resp" | tail -1)

    if [ "$http_status" -ge 400 ]; then
        pass "Wrong password returns HTTP $http_status"
    else
        fail "Wrong password should return 4xx, got $http_status"
    fi
}

# ── Step 5: /auth/me ────────────────────────────────────────────────────

step_me() {
    bold "Step 5: GET /auth/me"

    local resp http_status
    resp=$(curl -sk --max-time 10 \
        -X GET "$BACKEND_URL/auth/me" \
        -H "Authorization: Bearer $TOKEN" \
        -w "\n%{http_code}" 2>/dev/null)

    http_status=$(echo "$resp" | tail -1)
    local body
    body=$(echo "$resp" | sed '$d')

    assert_status 200 "$http_status" "GET /auth/me returns 200"
    assert_json_has "$body" "email" "Response has email"
    assert_json_has "$body" "user_id" "Response has user_id"
}

# ── Step 6: Upload file ─────────────────────────────────────────────────

step_upload() {
    bold "Step 6: Upload file to storage"

    local content="Hello from e2e shell test at $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    local resp http_status
    resp=$(curl -sk --max-time 10 \
        -X PUT "$BACKEND_URL/storage/e2e-test-file.txt" \
        -H "Authorization: Bearer $TOKEN" \
        -H "Content-Type: text/plain" \
        -d "$content" \
        -w "\n%{http_code}" 2>/dev/null)

    http_status=$(echo "$resp" | tail -1)

    if [ "$http_status" = "404" ]; then
        fail "Upload — storage not configured (RUSTFS_ENDPOINT not set?)"
        return
    fi

    if [ "$http_status" = "201" ] || [ "$http_status" = "200" ]; then
        pass "PUT /storage/e2e-test-file.txt returns $http_status"
    else
        fail "PUT /storage/e2e-test-file.txt (expected 201, got $http_status)"
    fi
}

# ── Step 7: Download file ───────────────────────────────────────────────

step_download() {
    bold "Step 7: Download file from storage"

    local resp http_status
    resp=$(curl -sk --max-time 10 \
        -X GET "$BACKEND_URL/storage/e2e-test-file.txt" \
        -H "Authorization: Bearer $TOKEN" \
        -w "\n%{http_code}" 2>/dev/null)

    http_status=$(echo "$resp" | tail -1)
    local body
    body=$(echo "$resp" | sed '$d')

    if [ "$http_status" = "404" ]; then
        fail "Download — storage not configured"
        return
    fi

    assert_status 200 "$http_status" "GET /storage/e2e-test-file.txt returns 200"
    assert_contains "$body" "e2e shell test" "Downloaded file contains test content"
}

# ── Step 8: Delete file ─────────────────────────────────────────────────

step_delete() {
    bold "Step 8: Delete file from storage"

    local resp http_status
    resp=$(curl -sk --max-time 10 \
        -X DELETE "$BACKEND_URL/storage/e2e-test-file.txt" \
        -H "Authorization: Bearer $TOKEN" \
        -w "\n%{http_code}" 2>/dev/null)

    http_status=$(echo "$resp" | tail -1)

    if [ "$http_status" = "404" ]; then
        fail "Delete — storage not configured"
        return
    fi

    if [ "$http_status" = "204" ] || [ "$http_status" = "200" ]; then
        pass "DELETE /storage/e2e-test-file.txt returns $http_status"
    else
        fail "DELETE /storage/e2e-test-file.txt (expected 204, got $http_status)"
    fi
}

# ── Step 9: Dashboard page ──────────────────────────────────────────────

step_dashboard() {
    bold "Step 9: Dashboard page"

    local resp
    resp=$(curl -sk --max-time 5 "$FRONTEND_URL/" 2>/dev/null)

    assert_contains "$resp" "FullStackHex" "Dashboard renders FullStackHex title"
}

# ── Step 10: Login page ─────────────────────────────────────────────────

step_login_page() {
    bold "Step 10: Login page"

    local resp
    resp=$(curl -sk --max-time 5 "$FRONTEND_URL/login" 2>/dev/null)

    assert_contains "$resp" "Sign in" "Login page renders 'Sign in'"
}

# ── Summary ─────────────────────────────────────────────────────────────

summary() {
    echo ""
    echo "═══════════════════════════════════════════"
    printf "  Results: %d passed, %d failed" "$PASS" "$FAIL"
    if [ "$FAIL" -eq 0 ]; then
        green " ✓"
        echo "═══════════════════════════════════════════"
        return 0
    else
        red " ✗"
        echo "═══════════════════════════════════════════"
        return 1
    fi
}

# ── Main ────────────────────────────────────────────────────────────────

main() {
    echo ""
    bold "FullStackHex E2E Test"
    echo "  Backend:  $BACKEND_URL"
    echo "  Frontend: $FRONTEND_URL"
    echo "  User:     $TEST_EMAIL"
    echo ""

    step_health
    step_register
    step_login
    step_wrong_password
    step_me
    step_upload
    step_download
    step_delete
    step_dashboard
    step_login_page

    summary
}

# Handle --help and start/stop flags
case "${1:-}" in
    --help|-h)
        echo "FullStackHex E2E Test"
        echo ""
        echo "Usage: ./tests/e2e.sh [OPTIONS]"
        echo ""
        echo "Options:"
        echo "  --help, -h    Show this help"
        echo ""
        echo "Env vars:"
        echo "  BACKEND_URL   Backend API URL (default: http://localhost:8001)"
        echo "  FRONTEND_URL  Frontend URL (default: http://localhost:4321)"
        exit 0
        ;;
    "")
        main
        ;;
    *)
        echo "Unknown option: $1"
        echo "Usage: ./tests/e2e.sh [--help]"
        exit 1
        ;;
esac
