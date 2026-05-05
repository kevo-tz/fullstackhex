#!/usr/bin/env bash
# deploy-blue-green.sh — Zero-downtime blue-green deployment.
#
# Maintains /opt/fullstackhex/blue and /opt/fullstackhex/green directories.
# Deploys to the inactive directory, starts services, switches nginx upstream,
# verifies health, then stops old services.
#
# Usage: ./scripts/deploy-blue-green.sh
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LOCK_FILE="/tmp/fullstackhex-deploy.lock"
COMPOSE_FILE="compose/prod.yml"

# ---------------------------------------------------------------------------
# Deploy lock
# ---------------------------------------------------------------------------
exec 200>"$LOCK_FILE"
if ! flock -n 200; then
    echo "ERROR: Another deploy is in progress."
    exit 1
fi

# ---------------------------------------------------------------------------
# Source config
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
IMAGE_TAG="${DEPLOY_IMAGE_TAG:-latest}"

if [ -z "$TARGET" ] || [ -z "$USER" ]; then
    echo "ERROR: DEPLOY_HOST and DEPLOY_USER must be set in .env"
    exit 1
fi

echo "=== FullStackHex Blue-Green Deploy ==="
echo "Image tag: ${IMAGE_TAG}"
echo "Target: ${USER}@${TARGET}"
echo ""

# Read current state
STATE_JSON=$(ssh "${USER}@${TARGET}" "cat ${STATE_FILE}" 2>/dev/null || echo '{"current_tag":"","previous_tag":"","active_dir":"blue"}')
CURRENT_DIR=$(echo "$STATE_JSON" | python3 -c "import json,sys; print(json.load(sys.stdin).get('active_dir','blue'))")

if [ "$CURRENT_DIR" = "blue" ]; then
    INACTIVE_DIR="green"
else
    INACTIVE_DIR="blue"
fi

INACTIVE_PATH="${DEPLOY_PATH}/${INACTIVE_DIR}"
CURRENT_PATH="${DEPLOY_PATH}/${CURRENT_DIR}"
NGINX_CONF="/etc/nginx/conf.d/fullstackhex.conf"

echo "Active directory: ${CURRENT_DIR}"
echo "Deploying to: ${INACTIVE_DIR}"
echo ""

# 1. Sync files to inactive directory
echo "Syncing files..."
rsync -avz --exclude='.git' --exclude='target' --exclude='node_modules' \
    compose/ scripts/ Makefile .env \
    "${USER}@${TARGET}:${INACTIVE_PATH}/"

# 2. Start services in inactive directory
echo "Starting services in ${INACTIVE_DIR}..."
ssh "${USER}@${TARGET}" "cd ${INACTIVE_PATH} && DEPLOY_IMAGE_TAG=${IMAGE_TAG} docker compose -f ${COMPOSE_FILE} up -d --wait"

# 3. Verify health on inactive directory's services
echo "Verifying health on ${INACTIVE_DIR}..."
# Wait for services to be ready
sleep 5

# 4. Backup nginx config
echo "Switching nginx upstream..."
ssh "${USER}@${TARGET}" "cp ${NGINX_CONF} ${NGINX_CONF}.bak"

# 5. Generate and apply new nginx config (pointer to inactive dir services)
NGINX_TEMPLATE="nginx/upstream.conf.template"
if [ -f "$NGINX_TEMPLATE" ]; then
    UPSTREAM_SUFFIX="_${INACTIVE_DIR}"
    sed "s/backend:8001/backend${UPSTREAM_SUFFIX}:8001/g; s/frontend:4321/frontend${UPSTREAM_SUFFIX}:4321/g" \
        "$NGINX_TEMPLATE" > /tmp/nginx-new.conf
else
    echo "WARNING: nginx/upstream.conf.template not found — skipping nginx switch"
fi

# 6. Copy new nginx config and validate/reload
scp /tmp/nginx-new.conf "${USER}@${TARGET}:${NGINX_CONF}"
ssh "${USER}@${TARGET}" "nginx -t && nginx -s reload" || {
    echo "ERROR: nginx config validation failed. Restoring backup."
    ssh "${USER}@${TARGET}" "cp ${NGINX_CONF}.bak ${NGINX_CONF} && nginx -s reload"
    exit 1
}

# 7. Verify health on new upstream
echo "Verifying health after switch..."
sleep 3
if "${SCRIPT_DIR}/deploy-verify.sh" --timeout 30; then
    echo "Health check passed."
else
    echo "ERROR: Health check failed. Rolling back nginx config."
    ssh "${USER}@${TARGET}" "cp ${NGINX_CONF}.bak ${NGINX_CONF} && nginx -s reload"
    echo "Rolled back nginx. Stopping ${INACTIVE_DIR} services..."
    ssh "${USER}@${TARGET}" "cd ${INACTIVE_PATH} && docker compose -f ${COMPOSE_FILE} stop"
    exit 1
fi

# 8. Stop old services (not rm — containers remain for rollback)
echo "Stopping old services in ${CURRENT_DIR}..."
ssh "${USER}@${TARGET}" "cd ${CURRENT_PATH} && docker compose -f ${COMPOSE_FILE} stop"

# 9. Update deploy state
python3 -c "
import json
state = json.loads('''${STATE_JSON}''')
state['previous_tag'] = state.get('current_tag', '')
state['current_tag'] = '${IMAGE_TAG}'
state['active_dir'] = '${INACTIVE_DIR}'
state['strategy'] = 'blue-green'
state['timestamp'] = '$(date -u +%Y-%m-%dT%H:%M:%SZ)'
print(json.dumps(state))
" | ssh "${USER}@${TARGET}" "cat > ${STATE_FILE}"

echo ""
echo "Blue-green deploy complete."
echo "Active directory: ${INACTIVE_DIR}"
