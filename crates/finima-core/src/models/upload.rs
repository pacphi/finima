use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::types::{FileFormat, UploadStatus};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Upload {
    pub id: Uuid,
    pub account_id: Uuid,
    pub filename: String,
    pub format: FileFormat,
    pub row_count: i32,
    pub imported_count: i32,
    pub duplicate_count: i32,
    pub status: UploadStatus,
    pub column_mapping: Option<serde_json::Value>,
    pub error_message: Option<String>,
    pub uploaded_at: DateTime<Utc>,
}
