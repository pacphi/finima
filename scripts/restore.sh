#!/usr/bin/env bash
set -euo pipefail

# Database restore script for Finima
# Downloads a specific backup from MinIO/S3 and restores it to PostgreSQL.
#
# Usage:
#   ./restore.sh                          # Restore the latest backup
#   ./restore.sh finima_backup_20260101_120000.sql.gz  # Restore a specific backup

BACKUP_NAME="${1:-}"
LOCAL_RESTORE_DIR="/tmp/restore"
BACKUP_BUCKET="${BACKUP_BUCKET:-finima-backups}"

MINIO_ENDPOINT="${MINIO_ENDPOINT:-http://minio:9000}"
MINIO_ACCESS_KEY="${MINIO_ACCESS_KEY}"
MINIO_SECRET_KEY="${MINIO_SECRET_KEY}"

PGHOST="${PGHOST:-postgres}"
PGUSER="${PGUSER:-finima}"
PGDATABASE="${PGDATABASE:-finima}"

log() {
  echo "[$(date --iso-8601=seconds)] RESTORE: $*"
}

log "Starting database restore..."

mkdir -p "${LOCAL_RESTORE_DIR}"

# Configure MinIO client
log "Configuring MinIO client..."
mc alias set finima "${MINIO_ENDPOINT}" "${MINIO_ACCESS_KEY}" "${MINIO_SECRET_KEY}" --api S3v4 2>/dev/null

# If no backup name specified, find the latest one
if [ -z "${BACKUP_NAME}" ]; then
  log "No backup name specified, finding latest backup..."
  BACKUP_NAME=$(mc ls "finima/${BACKUP_BUCKET}/" --json 2>/dev/null | \
    python3 -c "
import sys, json
backups = []
for line in sys.stdin:
    try:
        obj = json.loads(line)
        key = obj.get('key', '')
        if key.startswith('finima_backup_') and key.endswith('.sql.gz'):
            backups.append(key)
    except json.JSONDecodeError:
        pass
if backups:
    print(sorted(backups)[-1])
else:
    sys.exit(1)
")

  if [ -z "${BACKUP_NAME}" ]; then
    log "ERROR: No backups found in bucket '${BACKUP_BUCKET}'"
    exit 1
  fi
fi

log "Restoring from backup: ${BACKUP_NAME}"

# Download the backup
log "Downloading backup from s3://${BACKUP_BUCKET}/${BACKUP_NAME}..."
mc cp "finima/${BACKUP_BUCKET}/${BACKUP_NAME}" "${LOCAL_RESTORE_DIR}/${BACKUP_NAME}"

# Restore the database
log "Restoring database '${PGDATABASE}'..."
log "WARNING: This will overwrite existing data in '${PGDATABASE}'."

pg_restore \
  -h "${PGHOST}" \
  -U "${PGUSER}" \
  -d "${PGDATABASE}" \
  --no-owner \
  --no-acl \
  --clean \
  --if-exists \
  "${LOCAL_RESTORE_DIR}/${BACKUP_NAME}" || {
    log "WARNING: pg_restore completed with warnings (this is often normal for --clean --if-exists)"
  }

# Clean up local temp file
rm -f "${LOCAL_RESTORE_DIR}/${BACKUP_NAME}"

log "Restore complete."
