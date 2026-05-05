#!/usr/bin/env bash
# canary-rollback.sh — Rollback canary deployment (stop canary, restore nginx).
set -euo pipefail

if [ -f .env ]; then set -a; source .env; set +a; fi

TARGET="${DEPLOY_HOST:-}"
USER="${DEPLOY_USER:-}"
DEPLOY_PATH="${DEPLOY_PATH:-/opt/fullstackhex}"
CANARY_PATH="${DEPLOY_PATH}/canary"
NGINX_CONF="/etc/nginx/conf.d/fullstackhex.conf"

echo "Rolling back canary..."

# Restore nginx backup
ssh "${USER}@${TARGET}" "
if [ -f ${NGINX_CONF}.bak ]; then
    cp ${NGINX_CONF}.bak ${NGINX_CONF}
    nginx -t && nginx -s reload
    echo 'nginx restored from backup'
else
    echo 'WARNING: no nginx backup found — skipping nginx restore'
fi
"

# Stop canary services
ssh "${USER}@${TARGET}" "cd ${CANARY_PATH} && docker compose -f compose/prod.yml stop"

echo "Canary rolled back."
