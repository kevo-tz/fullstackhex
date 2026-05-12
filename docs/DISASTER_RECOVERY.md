# Disaster Recovery Guide

## PostgreSQL

### Backup

```bash
./scripts/backup.sh
```

Creates `.backup/pg_dump-<timestamp>.sql` using `pg_dump --clean --if-exists`.

### Restore

```bash
./scripts/restore.sh .backup <timestamp>
```

Restores from a specific backup timestamp. Uses `psql` to replay the dump.

### Point-in-time recovery

For production, enable WAL archiving in `postgresql.conf` and use `pg_basebackup` + WAL segments for PITR. This is not configured in the default dev setup.

## Redis

### Backup

`scripts/backup.sh` runs `redis-cli SAVE` and copies `dump.rdb` from the container.

### Restore

```bash
./scripts/restore.sh .backup <timestamp>
```

Copies the RDB file back into the container and reloads via `DEBUG RELOAD`.

### AOF persistence

Redis is configured with `--appendonly yes` by default. AOF files are stored in the `redis_data` volume and provide crash-safe persistence with second-level granularity.

## RustFS (S3-compatible Storage)

### Data location

RustFS stores data in the `rustfs_data` Docker volume.

### Backup

```bash
docker run --rm -v rustfs_data:/data -v .backup:/backup alpine tar czf /backup/rustfs-<timestamp>.tar.gz -C /data .
```

### Restore

```bash
docker run --rm -v rustfs_data:/data -v .backup:/backup alpine tar xzf /backup/rustfs-<timestamp>.tar.gz -C /data
```

## Container Failure Recovery

```bash
# Restart a single service
docker compose -f compose/dev.yml restart <service>

# Rebuild and restart
docker compose -f compose/dev.yml up -d --build <service>

# Full restart
make down && make dev
```

## Horizontal Scaling

The Rust API (Axum) is stateless and can be scaled horizontally behind nginx. The Python sidecar is stateful only via its Unix socket — scale by adding sidecar instances with separate socket paths. PostgreSQL and Redis are the bottleneck — use read replicas and Redis Cluster for production scaling.

## Reference

- `scripts/backup.sh` — automated backup
- `scripts/restore.sh` — automated restore
- `docs/INFRASTRUCTURE.md` — service architecture
