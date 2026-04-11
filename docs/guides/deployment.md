# Deployment Guide

This guide covers deploying Finima to a production server using Docker Compose.

## Prerequisites

### Server Requirements

| Resource | Minimum               | Recommended                              |
| -------- | --------------------- | ---------------------------------------- |
| CPU      | 2 cores               | 4+ cores                                 |
| RAM      | 4 GB                  | 8+ GB (16 GB if running Ollama with GPU) |
| Disk     | 20 GB                 | 50+ GB (Ollama models are 5-15 GB each)  |
| OS       | Any Linux with Docker | Ubuntu 24.04+ or Debian 12+              |

### Required Software

- Docker Engine 24+ with Compose V2
- A domain name with DNS pointing to your server (for auto-TLS)
- Git (to clone the repository)

### Optional

- NVIDIA GPU + nvidia-container-toolkit (for Ollama acceleration)

## Production Setup

### 1. Clone and Configure

```sh
git clone https://github.com/pacphi/finima.git
cd finima
```

### 2. Create Environment File

Copy the example and fill in production values:

```sh
cp .env.example .env
```

Edit `.env` with your production secrets:

```sh
# Database
POSTGRES_USER=finima
POSTGRES_PASSWORD=<strong-random-password>
POSTGRES_DB=finima

# Authentication
JWT_SECRET=<random-string-at-least-32-characters>

# Email (Resend.com)
RESEND_API_KEY=re_your_production_key

# Object Storage (MinIO)
MINIO_ROOT_USER=<minio-admin-user>
MINIO_ROOT_PASSWORD=<minio-admin-password-at-least-8-chars>

# Domain (used by Caddy for auto-TLS)
DOMAIN=finima.example.com

# CORS (must match your frontend domain)
APP__CORS__ALLOWED_ORIGINS=https://finima.example.com
```

### Required Environment Variables

| Variable                     | Purpose                   | Example                      |
| ---------------------------- | ------------------------- | ---------------------------- |
| `POSTGRES_PASSWORD`          | Database password         | Random, 20+ characters       |
| `JWT_SECRET`                 | JWT token signing         | Random, 32+ characters       |
| `RESEND_API_KEY`             | Magic link email delivery | `re_...` from resend.com     |
| `MINIO_ROOT_USER`            | MinIO admin username      | `finima-admin`               |
| `MINIO_ROOT_PASSWORD`        | MinIO admin password      | Random, 20+ characters       |
| `DOMAIN`                     | Your server domain        | `finima.example.com`         |
| `APP__CORS__ALLOWED_ORIGINS` | Allowed frontend origins  | `https://finima.example.com` |

### 3. Start the Production Stack

```sh
make docker-prod
```

This starts all services: PostgreSQL, Ollama, MinIO, backend, frontend, Caddy
reverse proxy, and the backup sidecar.

### 4. Verify Deployment

```sh
curl https://finima.example.com/health
```

Expected response:

```json
{ "status": "healthy", "version": "0.1.0", "db": "ok" }
```

## TLS/HTTPS

Caddy handles TLS automatically. It obtains and renews Let's Encrypt
certificates for the domain configured in the `DOMAIN` environment variable.

### Caddyfile

The default Caddyfile routes traffic as follows:

```text
{$DOMAIN:localhost} {
    reverse_proxy /api/* backend:3000
    reverse_proxy /ws/* backend:3000
    reverse_proxy frontend:80
}
```

- `/api/*` and `/ws/*` route to the backend on port 3000
- All other requests route to the frontend (nginx on port 80)

### Custom Domain

Set the `DOMAIN` environment variable before starting the stack. Caddy reads it
via the `{$DOMAIN}` placeholder. Ensure your DNS A record points to the server
IP before starting, so Caddy can complete the ACME challenge.

### Local/No-TLS Mode

If `DOMAIN` is unset or set to `localhost`, Caddy serves over HTTP on port 80
without TLS. This is useful for testing the production compose file locally.

## Ollama Setup

Ollama provides local LLM inference for transaction categorization and article
summarization.

### GPU Acceleration

The docker-compose production file reserves all NVIDIA GPUs by default. Ensure
you have:

1. NVIDIA drivers installed on the host
2. `nvidia-container-toolkit` installed
3. Docker configured to use the NVIDIA runtime

### Pull the Model

After starting the stack, pull the categorization model:

```sh
docker compose -f docker-compose.prod.yml exec ollama ollama pull gemma4:26b-a4b-it-q4_K_M
```

### CPU-Only Fallback

If no GPU is available, remove the `deploy.resources.reservations.devices`
section from the `ollama` service in `docker-compose.prod.yml`. Ollama will
run on CPU, but inference will be significantly slower. Consider using a smaller
model:

```sh
docker compose -f docker-compose.prod.yml exec ollama ollama pull gemma3:4b-it-qat
```

Update the model name in your environment:

```sh
APP__LLM__OLLAMA__MODEL=gemma3:4b-it-qat
```

## MinIO Storage

MinIO provides S3-compatible object storage for file uploads and database
backups.

### Bucket Auto-Creation

The backend creates the upload bucket (`finima-uploads`) automatically on first
use. The backup sidecar creates `finima-backups` on its first run.

### Production Credentials

Set `MINIO_ROOT_USER` and `MINIO_ROOT_PASSWORD` in `.env`. These are passed to
both MinIO and the backend. The MinIO console is not exposed externally in the
production compose file -- access it via SSH tunnel if needed:

```sh
ssh -L 9001:localhost:9001 your-server
# then open http://localhost:9001
```

## Database Operations

### Migrations

Migrations run automatically when the backend starts. They are located in
`crates/finima-db/src/migrations/` and executed in order by filename prefix.

No manual migration step is needed during deployment.

### Backups

The production stack includes a backup sidecar that runs `pg_dump` daily and
stores compressed backups in MinIO (`finima-backups` bucket). Backups are
retained for 30 days by default (configurable via `BACKUP_RETENTION_DAYS`).

Run a manual backup:

```sh
make backup
```

For detailed backup and restore procedures, see
[the backup guide](../backup-guide.md) (if available) or inspect
`scripts/backup.sh`.

## Monitoring

### SigNoz Observability

Start the full observability stack (SigNoz + OpenTelemetry Collector +
ClickHouse):

```sh
make observability
```

This overlays `docker-compose.observability.yml` on top of the dev compose. The
OTel Collector scrapes the backend `/metrics` endpoint and forwards data to
SigNoz.

Access the SigNoz dashboard at `http://your-server:3301`.

For detailed setup and dashboard configuration, see
[the observability guide](../observability-guide.md) (if available).

### Health Endpoint

`GET /health` returns the application and database status. Use it for load
balancer health checks and uptime monitoring.

```json
{ "status": "healthy", "version": "0.1.0", "db": "ok" }
```

Returns HTTP 503 if the database is unreachable.

### Metrics Endpoint

`GET /metrics` returns Prometheus-format metrics including:

- `http_requests_total` -- request count by method, path, status
- `http_request_duration_seconds` -- request latency histogram
- `http_requests_in_flight` -- current concurrent requests
- `error_rate_5xx` -- 5xx error counter for error budget tracking

## Scaling Considerations

### Connection Pool

The default pool is 10 connections (dev) / 25 connections (prod). Tune via:

```yaml
# config/production.yaml
database:
  max_connections: 50
```

Or via environment variable: `APP__DATABASE__MAX_CONNECTIONS=50`.

### Rate Limiting

The magic-link endpoint is rate-limited to 5 requests per minute per IP address.
This is configured in the router and cannot be changed via config at this time.

### Ollama Resources

Ollama is the most resource-intensive service. For production:

- Allocate at least 8 GB RAM for the `gemma4:26b` model
- Use a GPU with 12+ GB VRAM for acceptable latency
- If resources are tight, use a smaller model like `gemma3:4b`

### Request Body Limits

- Default: 1 MB for all endpoints
- File uploads: 50 MB (configured on upload routes)

## Updating

### Standard Update

```sh
cd finima

# Pull latest images
docker compose -f docker-compose.prod.yml pull

# Restart with new images (migrations run automatically)
make docker-prod
```

### Rollback

If a deployment fails:

```sh
# Stop the stack
make docker-prod-down

# Pin to a specific image tag in docker-compose.prod.yml
# e.g., image: ghcr.io/pacphi/finima-backend:v0.1.0

# Restart
make docker-prod
```

If a migration needs reverting (rare, requires the CLI):

```sh
make migrate-revert
```

## Security Checklist

Before going live, verify each item:

- [ ] `JWT_SECRET` is a unique random string (not `change-me-in-production`)
- [ ] `POSTGRES_PASSWORD` is strong and unique
- [ ] `MINIO_ROOT_USER` and `MINIO_ROOT_PASSWORD` are changed from defaults
- [ ] `RESEND_API_KEY` is set to a valid production key
- [ ] `APP__CORS__ALLOWED_ORIGINS` lists only your production domain(s)
- [ ] `DOMAIN` is set and DNS points to your server
- [ ] Backend container runs as non-root user (`finima`)
- [ ] Frontend container runs as non-root user (`nginx`)
- [ ] MinIO console port (9001) is not exposed to the internet
- [ ] SigNoz port (3301) is not exposed to the internet (or is behind auth)
- [ ] HSTS header is active (automatic when `APP_ENV=production`)
- [ ] Security headers are set (X-Content-Type-Options, X-Frame-Options)
