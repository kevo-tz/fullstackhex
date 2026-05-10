#!/usr/bin/env bash
set -euo pipefail
# backup.sh — Backup PostgreSQL database and Redis data.
# Usage: ./scripts/backup.sh [backup-dir]

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/config.sh"

BACKUP_DIR="${1:-$REPO_ROOT/.backup}"
mkdir -p "$BACKUP_DIR"
TIMESTAMP="$(date -u +%Y%m%d-%H%M%S)"

echo "Backing up to $BACKUP_DIR ..."

# PostgreSQL backup
echo "  → postgres"
docker compose -f compose/dev.yml --env-file .env exec -T postgres \
  pg_dump -U "$POSTGRES_USER" -d "$POSTGRES_DB" --clean --if-exists \
  > "$BACKUP_DIR/pg_dump-$TIMESTAMP.sql"

# Redis backup (SAVE + copy dump.rdb)
echo "  → redis"
docker compose -f compose/dev.yml --env-file .env exec -T redis \
  redis-cli SAVE
docker compose -f compose/dev.yml --env-file .env cp \
  redis:/data/dump.rdb "$BACKUP_DIR/redis_dump-$TIMESTAMP.rdb" 2>/dev/null || echo "    (rdb copy skipped)"

echo "Backup complete: $BACKUP_DIR"
ls -lh "$BACKUP_DIR"/*-"$TIMESTAMP"*
