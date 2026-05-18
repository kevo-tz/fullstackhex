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
$COMPOSE_DEV exec -T postgres \
  pg_dump -U "$POSTGRES_USER" -d "$POSTGRES_DB" --clean --if-exists \
  > "$BACKUP_DIR/pg_dump-$TIMESTAMP.sql"

# Redis backup (SAVE + copy dump.rdb)
echo "  → redis"
$COMPOSE_DEV exec -T redis \
  redis-cli SAVE
if ! $COMPOSE_DEV cp redis:/data/dump.rdb "$BACKUP_DIR/redis_dump-$TIMESTAMP.rdb"; then
    echo "    (rdb copy skipped — redis-cli SAVE may not have completed)" >&2
fi

echo "Backup complete: $BACKUP_DIR"
ls -lh "$BACKUP_DIR"/*-"$TIMESTAMP"*
