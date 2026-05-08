# Deploy

Production deployment with blue-green, canary, rollback, and health verification.

## Configuration

| Env Var | Description |
|---------|-------------|
| `DEPLOY_HOST` | Target server hostname/IP. |
| `DEPLOY_USER` | SSH user on target server. |
| `DEPLOY_PATH` | Deployment directory on target. |

## Deploy Lock

All deploy commands use `flock`-based locking to prevent concurrent deployments. The lock file is at `.deploy-state/lock`.

## Commands

### Blue-Green

```bash
make blue-green
```

Deploys to the inactive environment (blue/green), runs health checks, then switches nginx upstream. Zero-downtime deployment.

### Canary

```bash
make canary          # Deploy to canary (10% traffic)
make canary-promote  # Promote canary to 100%
make canary-rollback # Roll back canary
```

Deploys a canary instance receiving 10% of traffic via nginx `split_clients`. Monitor the canary, then promote or roll back.

### Rollback

```bash
make rollback
```

Switches back to the previous deployment. Relies on the deploy state file at `.deploy-state/current`.

### Health Verification

```bash
make deploy-verify
```

Polls health endpoints until all services report healthy or timeout. Used automatically by blue-green and canary before traffic switch.

## Nginx Config

| Template | Purpose |
|----------|---------|
| \`compose/nginx/upstream.conf.template\` | Blue-green upstream switching. |
| \`compose/nginx/canary.conf\` | Canary traffic split (10/90 default). |

## Production Compose

`compose/prod.yml` includes:
- nginx reverse proxy with TLS termination
- Resource limits per service
- Health checks on all containers
- Prometheus, Grafana, Alertmanager
- Redis and PostgreSQL exporters

```bash
make prod-up    # Start production stack
make prod-down  # Stop production stack
```

## State File

`.deploy-state/` tracks the current deployment state:
- `current` — active environment (blue/green).
- `lock` — deploy mutex.
- `canary_active` — whether a canary is deployed.

## Safety

- Deploy lock prevents concurrent deploys.
- Health verification gates traffic switching.
- Rollback is a single command — no manual steps.
- Canary limits blast radius to 10% of traffic.
