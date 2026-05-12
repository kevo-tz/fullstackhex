#!/usr/bin/env bash
set -euo pipefail
# restore.sh — Restore PostgreSQL database and Redis data from a backup.
# Usage: ./scripts/restore.sh <backup-dir> <timestamp>
# Example: ./scripts/restore.sh .backup 20260510-120000

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/config.sh"

if [ $# -lt 2 ]; then
  echo "Usage: $0 <backup-dir> <timestamp>"
  echo "Example: $0 .backup 20260510-120000"
  echo "Available backups:"
  find "$REPO_ROOT/.backup/" -maxdepth 1 -printf '- %f\n' 2>/dev/null || echo "  (no backups found)"
  exit 1
fi

BACKUP_DIR="$1"
TIMESTAMP="$2"
PG_FILE="$BACKUP_DIR/pg_dump-$TIMESTAMP.sql"
REDIS_FILE="$BACKUP_DIR/redis_dump-$TIMESTAMP.rdb"

echo "Restoring from $BACKUP_DIR (timestamp: $TIMESTAMP)..."
echo ""

# PostgreSQL restore
if [ -f "$PG_FILE" ]; then
  echo "  → restoring postgres from $PG_FILE"
  $COMPOSE_DEV exec -T postgres \
    psql -U "$POSTGRES_USER" -d "$POSTGRES_DB" < "$PG_FILE"
  echo "    postgres restore complete"
else
  echo "  → postgres: no backup found at $PG_FILE (skipping)"
fi

# Redis restore
if [ -f "$REDIS_FILE" ]; then
  echo "  → restoring redis from $REDIS_FILE"
  $COMPOSE_DEV cp "$REDIS_FILE" redis:/data/dump.rdb
  $COMPOSE_DEV exec -T redis redis-cli CONFIG SET dir /data
  $COMPOSE_DEV exec -T redis redis-cli DEBUG RELOAD
  echo "    redis restore complete"
else
  echo "  → redis: no backup found at $REDIS_FILE (skipping)"
fi

echo ""
echo "Restore complete."
