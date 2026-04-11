use aws_credential_types::Credentials;
use aws_sdk_s3::config::{BehaviorVersion, Region};
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::Client;

use crate::config::S3Config;

/// S3-compatible object storage client.
///
/// Works with AWS S3, MinIO, Azure Blob (via S3 gateway), and Google Cloud
/// Storage (via S3 interop). The `force_path_style` option must be `true`
/// for MinIO and most self-hosted backends.
#[derive(Clone)]
pub struct ObjectStorage {
    client: Client,
    bucket: String,
}

/// Errors that can occur during object storage operations.
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("Failed to create bucket: {0}")]
    CreateBucket(String),

    #[error("Failed to put object '{key}': {source}")]
    PutObject {
        key: String,
        source: aws_sdk_s3::error::SdkError<aws_sdk_s3::operation::put_object::PutObjectError>,
    },

    #[error("Failed to get object '{key}': {source}")]
    GetObject {
        key: String,
        source: aws_sdk_s3::error::SdkError<aws_sdk_s3::operation::get_object::GetObjectError>,
    },

    #[error("Failed to read object body: {0}")]
    ReadBody(String),

    #[error("Failed to delete object '{key}': {source}")]
    DeleteObject {
        key: String,
        source:
            aws_sdk_s3::error::SdkError<aws_sdk_s3::operation::delete_object::DeleteObjectError>,
    },
}

impl ObjectStorage {
    /// Build a new `ObjectStorage` from the application's S3 configuration.
    ///
    /// This also attempts to create the configured bucket if it does not
    /// already exist (idempotent for MinIO / AWS).
    pub async fn new(config: &S3Config) -> Result<Self, StorageError> {
        let credentials = Credentials::new(
            &config.access_key_id,
            &config.secret_access_key,
            None, // session token
            None, // expiry
            "finima-static-credentials",
        );

        let s3_config = aws_sdk_s3::Config::builder()
            .behavior_version(BehaviorVersion::latest())
            .region(Region::new(config.region.clone()))
            .endpoint_url(&config.endpoint_url)
            .credentials_provider(credentials)
            .force_path_style(config.force_path_style)
            .build();

        let client = Client::from_conf(s3_config);

        let storage = Self {
            client,
            bucket: config.bucket.clone(),
        };

        storage.ensure_bucket().await?;

        Ok(storage)
    }

    /// Create the bucket if it does not already exist.
    async fn ensure_bucket(&self) -> Result<(), StorageError> {
        // HEAD the bucket first; if it succeeds the bucket exists.
        let exists = self
            .client
            .head_bucket()
            .bucket(&self.bucket)
            .send()
            .await
            .is_ok();

        if !exists {
            self.client
                .create_bucket()
                .bucket(&self.bucket)
                .send()
                .await
                .map_err(|e| StorageError::CreateBucket(e.to_string()))?;

            tracing::info!(bucket = %self.bucket, "Created S3 bucket");
        }

        Ok(())
    }

    /// Upload an object and return the key it was stored under.
    pub async fn put_object(
        &self,
        key: &str,
        data: Vec<u8>,
        content_type: &str,
    ) -> Result<String, StorageError> {
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .body(ByteStream::from(data))
            .content_type(content_type)
            .send()
            .await
            .map_err(|e| StorageError::PutObject {
                key: key.to_string(),
                source: e,
            })?;

        tracing::debug!(key = %key, bucket = %self.bucket, "Stored object in S3");

        Ok(key.to_string())
    }

    /// Retrieve an object's bytes by key.
    pub async fn get_object(&self, key: &str) -> Result<Vec<u8>, StorageError> {
        let resp = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map_err(|e| StorageError::GetObject {
                key: key.to_string(),
                source: e,
            })?;

        let bytes = resp
            .body
            .collect()
            .await
            .map_err(|e| StorageError::ReadBody(e.to_string()))?
            .into_bytes()
            .to_vec();

        Ok(bytes)
    }

    /// Delete an object by key.
    pub async fn delete_object(&self, key: &str) -> Result<(), StorageError> {
        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map_err(|e| StorageError::DeleteObject {
                key: key.to_string(),
                source: e,
            })?;

        tracing::debug!(key = %key, bucket = %self.bucket, "Deleted object from S3");

        Ok(())
    }
}
