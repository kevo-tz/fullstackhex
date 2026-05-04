#!/usr/bin/env bash
# deploy-canary.sh — Canary deployment with nginx split_clients routing.
#
# Deploys new version to a canary directory and configures nginx to route
# 10% of traffic to the canary. Manual promotion only.
#
# Usage: ./scripts/deploy-canary.sh
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LOCK_FILE="/tmp/fullstackhex-deploy.lock"
COMPOSE_FILE="compose/prod.yml"
CANARY_WEIGHT="${CANARY_WEIGHT:-10}"

# ---------------------------------------------------------------------------
# Deploy lock
# ---------------------------------------------------------------------------
exec 200>"$LOCK_FILE"
if ! flock -n 200; then
    echo "ERROR: Another deploy is in progress."
    exit 1
fi

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
CANARY_PATH="${DEPLOY_PATH}/canary"
NGINX_CONF="/etc/nginx/conf.d/fullstackhex.conf"
CANARY_CONF="nginx/canary.conf"

if [ -z "$TARGET" ] || [ -z "$USER" ]; then
    echo "ERROR: DEPLOY_HOST and DEPLOY_USER must be set in .env"
    exit 1
fi

echo "=== FullStackHex Canary Deploy ==="
echo "Image tag: ${IMAGE_TAG}"
echo "Canary weight: ${CANARY_WEIGHT}%"
echo "Target: ${USER}@${TARGET}"
echo ""

# Sync files to canary directory
echo "Syncing files to canary..."
rsync -avz --exclude='.git' --exclude='target' --exclude='node_modules' \
    compose/ scripts/ Makefile .env \
    "${USER}@${TARGET}:${CANARY_PATH}/"

# Start canary services
echo "Starting canary services..."
ssh "${USER}@${TARGET}" "cd ${CANARY_PATH} && DEPLOY_IMAGE_TAG=${IMAGE_TAG} docker compose -f ${COMPOSE_FILE} up -d --wait"

sleep 5

# Verify canary health
echo "Verifying canary health..."
ssh "${USER}@${TARGET}" "curl -sk --max-time 5 http://localhost:8001/health | python3 -c \"import json,sys; d=json.load(sys.stdin); assert d.get('status')=='ok','canary unhealthy'\"" 2>/dev/null || {
    echo "ERROR: Canary health check failed. Stopping canary."
    ssh "${USER}@${TARGET}" "cd ${CANARY_PATH} && docker compose -f ${COMPOSE_FILE} stop"
    exit 1
}

# Backup and apply nginx canary config
echo "Applying nginx canary routing (${CANARY_WEIGHT}%)..."
ssh "${USER}@${TARGET}" "cp ${NGINX_CONF} ${NGINX_CONF}.bak"

# Generate canary nginx config
cat > /tmp/canary-nginx.conf << NGINX
split_clients "\$request_uri" \$backend_upstream {
    ${CANARY_WEIGHT}%    canary_backend;
    *      primary_backend;
}

upstream primary_backend {
    server backend:8001;
}

upstream canary_backend {
    server backend-canary:8001;
}
NGINX

scp /tmp/canary-nginx.conf "${USER}@${TARGET}:${NGINX_CONF}"

# Validate and reload
ssh "${USER}@${TARGET}" "nginx -t && nginx -s reload" || {
    echo "ERROR: nginx validation failed. Restoring backup."
    ssh "${USER}@${TARGET}" "cp ${NGINX_CONF}.bak ${NGINX_CONF} && nginx -s reload"
    echo "Stopping canary services..."
    ssh "${USER}@${TARGET}" "cd ${CANARY_PATH} && docker compose -f ${COMPOSE_FILE} stop"
    exit 1
}

# Update deploy state
STATE_JSON=$(ssh "${USER}@${TARGET}" "cat ${STATE_FILE}" 2>/dev/null || echo '{}')
python3 -c "
import json
state = json.loads('''${STATE_JSON}''')
state['canary_tag'] = '${IMAGE_TAG}'
state['canary_weight'] = ${CANARY_WEIGHT}
state['strategy'] = 'canary'
state['timestamp'] = '$(date -u +%Y-%m-%dT%H:%M:%SZ)'
print(json.dumps(state))
" | ssh "${USER}@${TARGET}" "cat > ${STATE_FILE}"

echo ""
echo "Canary deployed (${CANARY_WEIGHT}% traffic)."
echo "  Promote: make canary-promote"
echo "  Rollback: make canary-rollback"
