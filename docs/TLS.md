# TLS Certificate Management

## Automatic Renewal

The certbot container (`compose/prod.yml`) runs a loop that attempts renewal every 12 hours:

```yaml
entrypoint: "/bin/sh -c 'trap exit TERM; while :; do certbot renew; sleep 12h; done'"
```

Certificates are stored in `./nginx/certs/` (mounted from `/etc/letsencrypt` inside the container).

## Manual Renewal

```bash
# Trigger renewal check
docker compose -f compose/prod.yml exec certbot certbot renew

# Reload nginx to use renewed certificates
docker compose -f compose/prod.yml exec nginx nginx -s reload
```

## Monitoring

Check certbot logs for renewal failures:

```bash
docker compose -f compose/prod.yml logs certbot
```

Add a Prometheus alert for certificates expiring within 30 days using the `certbot` exporter or an external TLS monitoring service.

## Certificate Paths

| File | Container Path | Host Path |
|------|---------------|-----------|
| Full chain | `/etc/letsencrypt/live/<domain>/fullchain.pem` | `./nginx/certs/live/<domain>/fullchain.pem` |
| Private key | `/etc/letsencrypt/live/<domain>/privkey.pem` | `./nginx/certs/live/<domain>/privkey.pem` |

Nginx reads certificates from `/etc/nginx/certs/` which maps to `./nginx/certs/` on the host.

## Initial Setup

```bash
# First-time certificate issuance (requires DNS records pointing at the server)
docker compose -f compose/prod.yml run --rm certbot certonly --webroot -w /var/lib/certbot -d <domain>
```

## Troubleshooting

- If renewal fails, check port 80 is reachable from the internet (certbot uses HTTP-01 challenge)
- Certificate files must be readable by nginx user (UID 101)
- After certificate change, nginx must be reloaded or restarted
