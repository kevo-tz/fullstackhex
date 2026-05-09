# Deploy

Production deployment — use `docker compose` directly. Previous Makefile targets for blue-green, canary, rollback, and health verification have been removed.

## Configuration

| Env Var | Description |
|---------|-------------|
| `DEPLOY_HOST` | Target server hostname/IP. |
| `DEPLOY_USER` | SSH user on target server. |
| `DEPLOY_PATH` | Deployment directory on target. |

## Manual Deploy

Sync files and start the production stack:

```bash
rsync -avz compose/ nginx/ .env "$DEPLOY_USER@$DEPLOY_HOST:$DEPLOY_PATH/"
ssh "$DEPLOY_USER@$DEPLOY_HOST" "cd $DEPLOY_PATH && docker compose -f compose/prod.yml up -d --wait"
```

## Stop Production

```bash
ssh "$DEPLOY_USER@$DEPLOY_HOST" "cd $DEPLOY_PATH && docker compose -f compose/prod.yml down"
```

## Nginx Config

| Template | Purpose |
|----------|---------|
| \`compose/nginx/upstream.conf.template\` | Blue-green upstream switching. |
| \`compose/nginx/canary.conf\` | Canary traffic split (10/90 default). |

## Production Compose

`compose/prod.yml` includes:
- nginx reverse proxy with TLS termination
- Redis, PostgreSQL, and Nginx Prometheus exporters
- Resource limits per service
- Health checks on all containers
- Certbot for automatic TLS renewal

## Deploy Lock

Use `flock` if you need to prevent concurrent deployments manually. The lock file convention is at `.deploy-state/lock`.

## Health Verification

Poll health endpoints until all services report healthy:

```bash
ssh "$DEPLOY_USER@$DEPLOY_HOST" "curl -f https://localhost/health"
```
