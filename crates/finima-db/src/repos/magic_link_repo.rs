use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use finima_core::errors::AppError;
use finima_core::models::MagicLink;

/// PostgreSQL repository for magic link operations.
#[derive(Clone)]
pub struct PgMagicLinkRepo {
    pool: PgPool,
}

impl PgMagicLinkRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Insert a new magic link record.
    pub async fn create_magic_link(
        &self,
        email: &str,
        token_hash: &str,
        expires_at: DateTime<Utc>,
    ) -> Result<MagicLink, AppError> {
        let link = sqlx::query_as::<_, MagicLink>(
            r#"
            INSERT INTO magic_links (id, email, token_hash, expires_at, used_at)
            VALUES ($1, $2, $3, $4, NULL)
            RETURNING id, email, token_hash, expires_at, used_at
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(email)
        .bind(token_hash)
        .bind(expires_at)
        .fetch_one(&self.pool)
        .await?;

        Ok(link)
    }

    /// Find a magic link by its token hash.
    pub async fn find_by_token_hash(
        &self,
        token_hash: &str,
    ) -> Result<Option<MagicLink>, AppError> {
        let link = sqlx::query_as::<_, MagicLink>(
            "SELECT id, email, token_hash, expires_at, used_at FROM magic_links WHERE token_hash = $1",
        )
        .bind(token_hash)
        .fetch_optional(&self.pool)
        .await?;

        Ok(link)
    }

    /// Mark a magic link as used by setting its used_at timestamp.
    pub async fn mark_used(&self, id: Uuid) -> Result<(), AppError> {
        sqlx::query("UPDATE magic_links SET used_at = NOW() WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}
