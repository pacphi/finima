#!/usr/bin/env bash
set -euo pipefail

# Database backup script for Finima
# Creates a compressed pg_dump and uploads to MinIO/S3-compatible storage.
# Designed to run inside a Docker container with PostgreSQL client tools.

TIMESTAMP=$(date +%Y%m%d_%H%M%S)
BACKUP_FILE="finima_backup_${TIMESTAMP}.sql.gz"
LOCAL_BACKUP_DIR="/tmp/backups"
BACKUP_BUCKET="${BACKUP_BUCKET:-finima-backups}"
BACKUP_RETENTION_DAYS="${BACKUP_RETENTION_DAYS:-30}"

MINIO_ENDPOINT="${MINIO_ENDPOINT:-http://minio:9000}"
MINIO_ACCESS_KEY="${MINIO_ACCESS_KEY}"
MINIO_SECRET_KEY="${MINIO_SECRET_KEY}"

PGHOST="${PGHOST:-postgres}"
PGUSER="${PGUSER:-finima}"
PGDATABASE="${PGDATABASE:-finima}"

log() {
  echo "[$(date --iso-8601=seconds)] BACKUP: $*"
}

log "Starting database backup..."

mkdir -p "${LOCAL_BACKUP_DIR}"

# Create compressed backup
log "Running pg_dump for database '${PGDATABASE}'..."
pg_dump \
  -h "${PGHOST}" \
  -U "${PGUSER}" \
  -d "${PGDATABASE}" \
  --no-owner \
  --no-acl \
  --format=custom \
  --compress=9 \
  -f "${LOCAL_BACKUP_DIR}/${BACKUP_FILE}"

BACKUP_SIZE=$(du -h "${LOCAL_BACKUP_DIR}/${BACKUP_FILE}" | cut -f1)
log "Backup created: ${BACKUP_FILE} (${BACKUP_SIZE})"

# Configure MinIO client
log "Configuring MinIO client..."
mc alias set finima "${MINIO_ENDPOINT}" "${MINIO_ACCESS_KEY}" "${MINIO_SECRET_KEY}" --api S3v4 2>/dev/null

# Create bucket if it does not exist
if ! mc ls "finima/${BACKUP_BUCKET}" >/dev/null 2>&1; then
  log "Creating bucket '${BACKUP_BUCKET}'..."
  mc mb "finima/${BACKUP_BUCKET}"
fi

# Upload backup
log "Uploading backup to s3://${BACKUP_BUCKET}/${BACKUP_FILE}..."
mc cp "${LOCAL_BACKUP_DIR}/${BACKUP_FILE}" "finima/${BACKUP_BUCKET}/${BACKUP_FILE}"

log "Upload complete."

# Clean up old backups beyond retention period
log "Pruning backups older than ${BACKUP_RETENTION_DAYS} days..."
CUTOFF_DATE=$(date -d "-${BACKUP_RETENTION_DAYS} days" +%Y%m%d 2>/dev/null || date -v-${BACKUP_RETENTION_DAYS}d +%Y%m%d)

mc ls "finima/${BACKUP_BUCKET}/" --json 2>/dev/null | \
  python3 -c "
import sys, json
for line in sys.stdin:
    try:
        obj = json.loads(line)
        key = obj.get('key', '')
        if key.startswith('finima_backup_') and key.endswith('.sql.gz'):
            date_part = key.replace('finima_backup_', '').split('_')[0]
            if date_part < '${CUTOFF_DATE}':
                print(key)
    except (json.JSONDecodeError, IndexError):
        pass
" | while read -r old_backup; do
  log "Deleting old backup: ${old_backup}"
  mc rm "finima/${BACKUP_BUCKET}/${old_backup}"
done

# Clean up local temp file
rm -f "${LOCAL_BACKUP_DIR}/${BACKUP_FILE}"

log "Backup complete. Next backup in 24 hours."
