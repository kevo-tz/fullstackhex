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

# Restore full primary nginx config from template (no suffix = primary)
NGINX_TEMPLATE="nginx/upstream.conf.template"
sed 's/\$upstream_suffix//g' "${NGINX_TEMPLATE}" > /tmp/promote-nginx.conf

scp /tmp/promote-nginx.conf "${USER}@${TARGET}:${NGINX_CONF}"

ssh "${USER}@${TARGET}" "nginx -t && nginx -s reload" || {
    echo "ERROR: nginx reload failed. Restoring backup."
    ssh "${USER}@${TARGET}" "cp ${NGINX_CONF}.bak ${NGINX_CONF} && nginx -s reload"
    exit 1
}

# Stop canary services
ssh "${USER}@${TARGET}" "cd ${CANARY_PATH} && docker compose -f compose/prod.yml stop"

echo "Canary promoted."
