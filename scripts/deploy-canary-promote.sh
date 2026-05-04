#!/usr/bin/env bash
# canary-promote.sh — Promote canary to primary (100% traffic).
set -euo pipefail

if [ -f .env ]; then set -a; source .env; set +a; fi

TARGET="${DEPLOY_HOST:-}"
USER="${DEPLOY_USER:-}"
DEPLOY_PATH="${DEPLOY_PATH:-/opt/fullstackhex}"
CANARY_PATH="${DEPLOY_PATH}/canary"
NGINX_CONF="/etc/nginx/conf.d/fullstackhex.conf"

echo "Promoting canary to primary..."
ssh "${USER}@${TARGET}" "cp ${NGINX_CONF} ${NGINX_CONF}.bak"

# Switch all traffic to primary backend (remove canary split)
cat > /tmp/promote-nginx.conf << NGINX
upstream backend {
    server backend:8001;
}
NGINX

scp /tmp/promote-nginx.conf "${USER}@${TARGET}:${NGINX_CONF}"

ssh "${USER}@${TARGET}" "nginx -t && nginx -s reload" || {
    echo "ERROR: nginx reload failed. Restoring backup."
    ssh "${USER}@${TARGET}" "cp ${NGINX_CONF}.bak ${NGINX_CONF} && nginx -s reload"
    exit 1
}

# Stop canary services
ssh "${USER}@${TARGET}" "cd ${CANARY_PATH} && docker compose -f compose/prod.yml stop"

echo "Canary promoted."
