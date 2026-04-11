use sqlx::PgPool;
use uuid::Uuid;

use finima_core::errors::AppError;
use finima_core::models::Upload;
use finima_core::types::{FileFormat, UploadStatus};

/// PostgreSQL implementation of upload repository operations.
#[derive(Clone)]
pub struct PgUploadRepo {
    pool: PgPool,
}

impl PgUploadRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create a new upload record in pending status.
    pub async fn create(
        &self,
        account_id: Uuid,
        filename: &str,
        format: FileFormat,
    ) -> Result<Upload, AppError> {
        let upload = sqlx::query_as::<_, Upload>(
            r#"
            INSERT INTO uploads (id, account_id, filename, format, row_count, imported_count,
                                 duplicate_count, status, uploaded_at)
            VALUES ($1, $2, $3, $4, 0, 0, 0, $5, NOW())
            RETURNING id, account_id, filename, format, row_count, imported_count,
                      duplicate_count, status, column_mapping, error_message, uploaded_at
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(account_id)
        .bind(filename)
        .bind(format.to_string())
        .bind(UploadStatus::Pending.to_string())
        .fetch_one(&self.pool)
        .await?;

        Ok(upload)
    }

    /// Update the status and optional count/error fields of an upload.
    pub async fn update_status(
        &self,
        id: Uuid,
        status: UploadStatus,
        row_count: Option<i32>,
        imported_count: Option<i32>,
        duplicate_count: Option<i32>,
        error_message: Option<&str>,
    ) -> Result<(), AppError> {
        sqlx::query(
            r#"
            UPDATE uploads
            SET status = $2,
                row_count = COALESCE($3, row_count),
                imported_count = COALESCE($4, imported_count),
                duplicate_count = COALESCE($5, duplicate_count),
                error_message = $6
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(status.to_string())
        .bind(row_count)
        .bind(imported_count)
        .bind(duplicate_count)
        .bind(error_message)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Store the user-confirmed column mapping for an upload.
    pub async fn update_column_mapping(
        &self,
        id: Uuid,
        mapping: serde_json::Value,
    ) -> Result<(), AppError> {
        sqlx::query("UPDATE uploads SET column_mapping = $2 WHERE id = $1")
            .bind(id)
            .bind(mapping)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Find an upload by ID.
    pub async fn find_by_id(&self, id: Uuid) -> Result<Upload, AppError> {
        let upload = sqlx::query_as::<_, Upload>(
            r#"
            SELECT id, account_id, filename, format, row_count, imported_count,
                   duplicate_count, status, column_mapping, error_message, uploaded_at
            FROM uploads
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await?;

        Ok(upload)
    }
}
