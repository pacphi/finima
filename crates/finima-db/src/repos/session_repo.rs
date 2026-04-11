use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use finima_core::errors::AppError;
use finima_core::models::Session;

/// PostgreSQL repository for session operations.
///
/// Sessions are server-side entities that bind a refresh token hash to a user.
/// Each session is single-use: consuming a refresh token invalidates the old
/// session and creates a new one. Logout revokes the session by deleting it.
#[derive(Clone)]
pub struct PgSessionRepo {
    pool: PgPool,
}

impl PgSessionRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Insert a new session record.
    pub async fn create_session(
        &self,
        user_id: Uuid,
        refresh_token_hash: &str,
        expires_at: DateTime<Utc>,
    ) -> Result<Session, AppError> {
        let session = sqlx::query_as::<_, Session>(
            r#"
            INSERT INTO sessions (id, user_id, token_hash, expires_at)
            VALUES ($1, $2, $3, $4)
            RETURNING id, user_id, token_hash, expires_at
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(user_id)
        .bind(refresh_token_hash)
        .bind(expires_at)
        .fetch_one(&self.pool)
        .await?;

        Ok(session)
    }

    /// Find a session by its refresh token hash.
    pub async fn find_by_refresh_hash(&self, hash: &str) -> Result<Option<Session>, AppError> {
        let session = sqlx::query_as::<_, Session>(
            "SELECT id, user_id, token_hash, expires_at FROM sessions WHERE token_hash = $1",
        )
        .bind(hash)
        .fetch_optional(&self.pool)
        .await?;

        Ok(session)
    }

    /// Delete a single session by ID (logout / token rotation).
    pub async fn delete_session(&self, session_id: Uuid) -> Result<(), AppError> {
        sqlx::query("DELETE FROM sessions WHERE id = $1")
            .bind(session_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Delete all sessions for a user (e.g. password change, account lockout).
    pub async fn delete_all_user_sessions(&self, user_id: Uuid) -> Result<(), AppError> {
        sqlx::query("DELETE FROM sessions WHERE user_id = $1")
            .bind(user_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Remove expired sessions and return the number of rows deleted.
    pub async fn cleanup_expired(&self) -> Result<u64, AppError> {
        let result = sqlx::query("DELETE FROM sessions WHERE expires_at < NOW()")
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore] // Requires a running PostgreSQL database
    fn test_session_repo() {
        // Integration test placeholder
    }
}
