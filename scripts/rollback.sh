#!/usr/bin/env bash
# rollback.sh — Rollback to the previous deployment version.
#
# Reads .deploy-state to find the previous tag, swaps current/previous,
# pulls images, restarts services, and verifies health.
#
# Usage: ./scripts/rollback.sh [--target HOST] [--user USER] [--path PATH]
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LOCK_FILE="/tmp/fullstackhex-deploy.lock"

# ---------------------------------------------------------------------------
# Deploy lock: prevent concurrent deploys
# ---------------------------------------------------------------------------
exec 200>"$LOCK_FILE"
if ! flock -n 200; then
    echo "ERROR: Another deploy or rollback is in progress. Try again later."
    exit 1
fi

# ---------------------------------------------------------------------------
# Source config from .env
# ---------------------------------------------------------------------------
if [ -f .env ]; then
    set -a
    source .env
    set +a
fi

TARGET="${DEPLOY_HOST:-}"
USER="${DEPLOY_USER:-}"
DEPLOY_PATH="${DEPLOY_PATH:-/opt/fullstackhex}"
STATE_FILE="${DEPLOY_PATH}/.deploy-state"

# ---------------------------------------------------------------------------
# Validate
# ---------------------------------------------------------------------------
if [ -z "$TARGET" ] || [ -z "$USER" ]; then
    echo "ERROR: DEPLOY_HOST and DEPLOY_USER must be set in .env"
    exit 1
fi

echo "=== FullStackHex Rollback ==="
echo "Target: ${USER}@${TARGET}"
echo ""

# Read current state
STATE_JSON=$(ssh "${USER}@${TARGET}" "cat ${STATE_FILE}" 2>/dev/null || echo "")
if [ -z "$STATE_JSON" ]; then
    echo "ERROR: No .deploy-state found on ${TARGET}."
    echo "       Deploy first with: make deploy"
    exit 1
fi

PREVIOUS_TAG=$(echo "$STATE_JSON" | python3 -c "import json,sys; print(json.load(sys.stdin).get('previous_tag',''))")
CURRENT_TAG=$(echo "$STATE_JSON" | python3 -c "import json,sys; print(json.load(sys.stdin).get('current_tag',''))")

if [ -z "$PREVIOUS_TAG" ]; then
    echo "ERROR: No previous deployment found (previous_tag is empty in .deploy-state)."
    echo "       There is nothing to roll back to."
    exit 1
fi

echo "Current tag: ${CURRENT_TAG}"
echo "Rolling back to: ${PREVIOUS_TAG}"
echo ""

# Backup the current state file
ssh "${USER}@${TARGET}" "cp ${STATE_FILE} ${STATE_FILE}.bak"

# Swap tags: previous becomes current, current becomes previous
python3 -c "
import json, sys
state = json.loads('''${STATE_JSON}''')
state['previous_tag'] = state['current_tag']
state['current_tag'] = '${PREVIOUS_TAG}'
state['timestamp'] = '$(date -u +%Y-%m-%dT%H:%M:%SZ)'
print(json.dumps(state))
" | ssh "${USER}@${TARGET}" "cat > ${STATE_FILE}"

# Pull images and restart
echo "Pulling images for tag ${PREVIOUS_TAG}..."
ssh "${USER}@${TARGET}" "cd ${DEPLOY_PATH} && DEPLOY_IMAGE_TAG=${PREVIOUS_TAG} docker compose -f compose/prod.yml pull"

echo "Restarting services..."
ssh "${USER}@${TARGET}" "cd ${DEPLOY_PATH} && DEPLOY_IMAGE_TAG=${PREVIOUS_TAG} docker compose -f compose/prod.yml up -d --wait"

# Verify health
echo ""
echo "Verifying health after rollback..."
if "${SCRIPT_DIR}/deploy-verify.sh" --timeout 60 --base-url "https://${TARGET}/health"; then
    echo "Rollback completed successfully."
else
    echo "ERROR: Health check failed after rollback."
    echo "Restoring previous state..."
    ssh "${USER}@${TARGET}" "cp ${STATE_FILE}.bak ${STATE_FILE}"
    echo "Re-reverting to ${CURRENT_TAG}..."
    ssh "${USER}@${TARGET}" "cd ${DEPLOY_PATH} && DEPLOY_IMAGE_TAG=${CURRENT_TAG} docker compose -f compose/prod.yml up -d --wait"
    exit 1
fi
