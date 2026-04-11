# Object Storage Setup

Finima uses S3-compatible object storage for file uploads (transaction CSVs, receipts, etc.) and database backups. By default, it runs a self-hosted MinIO instance, but you can point it at any S3-compatible provider.

## MinIO (default, self-hosted)

MinIO runs automatically as part of the dev Docker Compose stack:

```bash
make docker-up
```

The MinIO console is available at <http://localhost:9001> with the default credentials:

- **User:** `finima`
- **Password:** `finima_dev`

The S3 API endpoint is <http://localhost:9000>.

### Configuration

Default values are set in `config/default.yaml`:

```yaml
s3:
  endpoint_url: 'http://localhost:9000'
  region: 'us-east-1'
  bucket: 'finima-uploads'
  access_key_id: 'finima'
  secret_access_key: 'finima_dev'
  force_path_style: true
```

In production, override these with environment variables:

| Variable                     | Description                | Example                           |
| ---------------------------- | -------------------------- | --------------------------------- |
| `APP__S3__ENDPOINT_URL`      | S3 API endpoint            | `http://minio:9000`               |
| `APP__S3__REGION`            | AWS region or MinIO region | `us-east-1`                       |
| `APP__S3__BUCKET`            | Bucket name for uploads    | `finima-uploads`                  |
| `APP__S3__ACCESS_KEY_ID`     | Access key                 | (from `MINIO_ROOT_USER`)          |
| `APP__S3__SECRET_ACCESS_KEY` | Secret key                 | (from `MINIO_ROOT_PASSWORD`)      |
| `APP__S3__FORCE_PATH_STYLE`  | Use path-style URLs        | `true` for MinIO, `false` for AWS |
| `MINIO_ROOT_USER`            | MinIO root username        | `finima`                          |
| `MINIO_ROOT_PASSWORD`        | MinIO root password        | (a strong password)               |

## AWS S3

To use AWS S3 instead of MinIO:

1. Create an S3 bucket in the AWS console.
2. Create an IAM user with `s3:PutObject`, `s3:GetObject`, `s3:DeleteObject`, and `s3:ListBucket` permissions on the bucket.
3. Set the following environment variables:

```bash
APP__S3__ENDPOINT_URL=""           # Leave empty for default AWS endpoint
APP__S3__REGION="us-west-2"        # Your bucket's region
APP__S3__BUCKET="your-bucket-name"
APP__S3__ACCESS_KEY_ID="AKIA..."
APP__S3__SECRET_ACCESS_KEY="..."
APP__S3__FORCE_PATH_STYLE="false"  # AWS uses virtual-hosted-style
```

4. Remove or do not start the `minio` service from Docker Compose.

## Azure Blob Storage

Azure Blob Storage exposes an S3-compatible endpoint via the Azure Storage REST API.

1. Create a Storage Account in the Azure portal.
2. Create a container (equivalent to a bucket).
3. Enable the S3-compatible endpoint (preview feature) or use a gateway like [MinIO Gateway for Azure](https://min.io/docs/minio/linux/operations/install-deploy-manage/migrate-fs-gateway.html).
4. Configure:

```bash
APP__S3__ENDPOINT_URL="https://<account>.blob.core.windows.net"
APP__S3__REGION="us-east-1"
APP__S3__BUCKET="your-container-name"
APP__S3__ACCESS_KEY_ID="<storage-account-name>"
APP__S3__SECRET_ACCESS_KEY="<storage-account-key>"
APP__S3__FORCE_PATH_STYLE="true"
```

## Google Cloud Storage

GCS provides an S3-compatible endpoint via the XML API.

1. Create a GCS bucket.
2. Create a service account with `Storage Object Admin` role on the bucket.
3. Generate HMAC keys for the service account in the GCS console under **Settings > Interoperability**.
4. Configure:

```bash
APP__S3__ENDPOINT_URL="https://storage.googleapis.com"
APP__S3__REGION="auto"
APP__S3__BUCKET="your-gcs-bucket"
APP__S3__ACCESS_KEY_ID="<hmac-access-id>"
APP__S3__SECRET_ACCESS_KEY="<hmac-secret>"
APP__S3__FORCE_PATH_STYLE="false"
```

## Environment Variable Reference

| Variable                     | Default                 | Description                |
| ---------------------------- | ----------------------- | -------------------------- |
| `MINIO_ROOT_USER`            | `finima`                | MinIO root username        |
| `MINIO_ROOT_PASSWORD`        | `finima_dev`            | MinIO root password        |
| `S3_REGION`                  | `us-east-1`             | S3 region                  |
| `S3_BUCKET`                  | `finima-uploads`        | Default bucket for uploads |
| `APP__S3__ENDPOINT_URL`      | `http://localhost:9000` | S3 API endpoint URL        |
| `APP__S3__REGION`            | `us-east-1`             | S3 region                  |
| `APP__S3__BUCKET`            | `finima-uploads`        | Bucket for file uploads    |
| `APP__S3__ACCESS_KEY_ID`     | `finima`                | S3 access key              |
| `APP__S3__SECRET_ACCESS_KEY` | `finima_dev`            | S3 secret key              |
| `APP__S3__FORCE_PATH_STYLE`  | `true`                  | Use path-style addressing  |
