#[cfg(feature = "axum")]
use axum::http::StatusCode;
#[cfg(feature = "axum")]
use axum::response::{IntoResponse, Response};
#[cfg(feature = "axum")]
use serde_json::json;

use thiserror::Error;

/// Application-wide error type.
#[derive(Debug, Error)]
pub enum AppError {
    #[error("Not found")]
    NotFound,

    #[error("Unauthorized")]
    Unauthorized,

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Internal error: {0}")]
    InternalError(String),

    #[error("Database error")]
    DatabaseError,

    #[error("LLM error")]
    LlmError,

    #[error("Parse error: {0}")]
    ParseError(String),
}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        tracing::error!(error = %err, "Database error");
        match err {
            sqlx::Error::RowNotFound => AppError::NotFound,
            sqlx::Error::Database(ref db_err) => {
                // PostgreSQL unique constraint violation
                if db_err.code().as_deref() == Some("23505") {
                    AppError::Conflict(db_err.message().to_string())
                } else {
                    AppError::DatabaseError
                }
            }
            _ => AppError::DatabaseError,
        }
    }
}

#[cfg(feature = "axum")]
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::NotFound => (StatusCode::NOT_FOUND, self.to_string()),
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, self.to_string()),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            AppError::Conflict(msg) => (StatusCode::CONFLICT, msg.clone()),
            AppError::InternalError(msg) => {
                tracing::error!(error = %msg, "Internal server error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                )
            }
            AppError::DatabaseError => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Database error".to_string(),
            ),
            AppError::LlmError => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "LLM processing error".to_string(),
            ),
            AppError::ParseError(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
        };

        let body = json!({
            "error": message,
        });

        (status, axum::Json(body)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sqlx_row_not_found_maps_to_not_found() {
        let err: AppError = sqlx::Error::RowNotFound.into();
        assert!(matches!(err, AppError::NotFound));
    }

    #[test]
    fn app_error_display() {
        assert_eq!(AppError::NotFound.to_string(), "Not found");
        assert_eq!(AppError::Unauthorized.to_string(), "Unauthorized");
        assert_eq!(
            AppError::BadRequest("missing field".into()).to_string(),
            "Bad request: missing field"
        );
        assert_eq!(
            AppError::Conflict("duplicate email".into()).to_string(),
            "Conflict: duplicate email"
        );
        assert_eq!(
            AppError::ParseError("invalid date".into()).to_string(),
            "Parse error: invalid date"
        );
    }
}
