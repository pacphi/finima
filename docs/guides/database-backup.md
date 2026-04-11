# Database Backup & Restore

Finima includes automated daily backups of the PostgreSQL database to S3-compatible storage (MinIO by default).

## How Backups Work

The production Docker Compose stack (`docker-compose.prod.yml`) includes a `backup` container that:

1. Runs `pg_dump` with custom format and maximum compression.
2. Uploads the dump to a MinIO/S3 bucket (`finima-backups` by default).
3. Prunes backups older than the configured retention period (default: 30 days).
4. Sleeps for 24 hours and repeats.

Backup files are named `finima_backup_YYYYMMDD_HHMMSS.sql.gz`.

## Running a Manual Backup

From the project root:

```bash
make backup
```

This executes `scripts/backup.sh` inside a temporary container with access to the database and MinIO.

## Restoring from Backup

### Restore the Latest Backup

```bash
docker compose -f docker-compose.prod.yml run --rm backup /scripts/restore.sh
```

### Restore a Specific Backup

```bash
docker compose -f docker-compose.prod.yml run --rm backup /scripts/restore.sh finima_backup_20260411_030000.sql.gz
```

### Local Restore (without Docker)

If you have `mc` (MinIO Client) and `pg_restore` installed locally:

```bash
# Download the backup
mc alias set finima http://localhost:9000 finima finima_dev
mc cp finima/finima-backups/finima_backup_20260411_030000.sql.gz ./backup.sql.gz

# Restore
pg_restore \
  -h localhost -U finima -d finima \
  --no-owner --no-acl --clean --if-exists \
  ./backup.sql.gz
```

## Configuration

All backup settings are configured via environment variables on the `backup` service in `docker-compose.prod.yml`:

| Variable                | Default                      | Description                |
| ----------------------- | ---------------------------- | -------------------------- |
| `PGHOST`                | `postgres`                   | PostgreSQL hostname        |
| `PGUSER`                | (from `POSTGRES_USER`)       | Database user              |
| `PGPASSWORD`            | (from `POSTGRES_PASSWORD`)   | Database password          |
| `PGDATABASE`            | `finima`                     | Database name              |
| `MINIO_ENDPOINT`        | `http://minio:9000`          | MinIO/S3 endpoint          |
| `MINIO_ACCESS_KEY`      | (from `MINIO_ROOT_USER`)     | S3 access key              |
| `MINIO_SECRET_KEY`      | (from `MINIO_ROOT_PASSWORD`) | S3 secret key              |
| `BACKUP_BUCKET`         | `finima-backups`             | Bucket for storing backups |
| `BACKUP_RETENTION_DAYS` | `30`                         | Days to keep old backups   |

## Verifying Backups

To list available backups:

```bash
# Via MinIO Client
mc alias set finima http://localhost:9000 finima finima_dev
mc ls finima/finima-backups/

# Via Docker
docker compose -f docker-compose.prod.yml run --rm backup mc ls finima/finima-backups/
```

To verify a backup is valid without restoring:

```bash
# Download and inspect
mc cp finima/finima-backups/finima_backup_20260411_030000.sql.gz ./backup.sql.gz
pg_restore --list ./backup.sql.gz
```

This prints the table of contents of the backup without modifying any database.

## Changing Retention

Set the `BACKUP_RETENTION_DAYS` environment variable in your `.env` file or directly in `docker-compose.prod.yml`:

```bash
# Keep 90 days of backups
BACKUP_RETENTION_DAYS=90
```

## Backup to External S3

To back up to AWS S3 or another provider instead of local MinIO, change the `MINIO_ENDPOINT`, `MINIO_ACCESS_KEY`, and `MINIO_SECRET_KEY` environment variables on the `backup` service to point at the external endpoint.
