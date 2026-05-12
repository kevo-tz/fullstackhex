# Secrets Rotation Guide

> **Cross-platform note:** The `sed -i` commands below work on Linux. On macOS, install GNU sed (`brew install gnu-sed`) and use `gsed -i 's/.../.../' .env`, or add an empty backup extension: `sed -i '' 's/.../.../' .env`.

## JWT_SECRET

Rotating the JWT secret invalidates ALL existing sessions — all users must re-authenticate.

```bash
# Generate new secret
openssl rand -hex 32

# Update .env
sed -i 's/JWT_SECRET=.*/JWT_SECRET=<new-secret>/' .env

# Restart backend
docker compose -f compose/prod.yml restart backend
```

## DATABASE_URL Password

```sql
-- Connect to PostgreSQL and alter password
ALTER USER app_user PASSWORD '<new-password>';

-- Update .env
sed -i 's/POSTGRES_PASSWORD=.*/POSTGRES_PASSWORD=<new-password>/' .env
sed -i 's|DATABASE_URL=postgres://app_user:[^@]*@|DATABASE_URL=postgres://app_user:<new-password>@|' .env

-- For dev, also update the exporter config
docker compose -f compose/dev.yml restart postgres postgres-exporter
```

## REDIS_PASSWORD

```bash
# Generate new password
openssl rand -hex 32

# Update .env
sed -i 's/REDIS_PASSWORD=.*/REDIS_PASSWORD=<new-password>/' .env

# Restart Redis and Redis-dependent services
docker compose -f compose/prod.yml restart redis redis-exporter backend py-api
```

## RUSTFS_ACCESS_KEY / RUSTFS_SECRET_KEY

```bash
# Update .env
sed -i 's/RUSTFS_ACCESS_KEY=.*/RUSTFS_ACCESS_KEY=<new-key>/' .env
sed -i 's/RUSTFS_SECRET_KEY=.*/RUSTFS_SECRET_KEY=<new-secret>/' .env

# Restart RustFS and dependent services
docker compose -f compose/prod.yml restart rustfs backend
```

## SIDECAR_SHARED_SECRET

The HMAC shared secret between the Rust backend and Python sidecar. Both must use the same value.

```bash
# Generate new secret
openssl rand -hex 32

# Update .env
sed -i 's/SIDECAR_SHARED_SECRET=.*/SIDECAR_SHARED_SECRET=<new-secret>/' .env

# Restart both services
docker compose -f compose/prod.yml restart backend py-api
```

## TLS Certificates (Certbot)

Certificates are auto-renewed every 12 hours via the certbot container's entrypoint loop:

```bash
# Manual renewal
docker compose -f compose/prod.yml exec certbot certbot renew

# Reload nginx to pick up new certificates
docker compose -f compose/prod.yml exec nginx nginx -s reload
```

See [TLS.md](./TLS.md) for details on certificate monitoring and renewal.
