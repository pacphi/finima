use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use finima_core::errors::AppError;
use finima_core::models::User;
use finima_core::traits::UserRepo;

/// PostgreSQL implementation of UserRepo.
#[derive(Clone)]
pub struct PgUserRepo {
    pool: PgPool,
}

impl PgUserRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl UserRepo for PgUserRepo {
    async fn create_user(&self, email: &str, display_name: &str) -> Result<User, AppError> {
        let user = sqlx::query_as::<_, User>(
            r#"
            INSERT INTO users (id, email, display_name, preferences, created_at, updated_at)
            VALUES ($1, $2, $3, $4, NOW(), NOW())
            RETURNING id, email, display_name, preferences, created_at, updated_at
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(email)
        .bind(display_name)
        .bind(serde_json::json!({}))
        .fetch_one(&self.pool)
        .await?;

        Ok(user)
    }

    async fn find_by_email(&self, email: &str) -> Result<Option<User>, AppError> {
        let user = sqlx::query_as::<_, User>(
            "SELECT id, email, display_name, preferences, created_at, updated_at FROM users WHERE email = $1",
        )
        .bind(email)
        .fetch_optional(&self.pool)
        .await?;

        Ok(user)
    }

    async fn find_by_id(&self, id: Uuid) -> Result<User, AppError> {
        let user = sqlx::query_as::<_, User>(
            "SELECT id, email, display_name, preferences, created_at, updated_at FROM users WHERE id = $1",
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await?;

        Ok(user)
    }

    async fn update_preferences(
        &self,
        id: Uuid,
        preferences: serde_json::Value,
    ) -> Result<User, AppError> {
        let user = sqlx::query_as::<_, User>(
            r#"
            UPDATE users
            SET preferences = $2, updated_at = NOW()
            WHERE id = $1
            RETURNING id, email, display_name, preferences, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(preferences)
        .fetch_one(&self.pool)
        .await?;

        Ok(user)
    }
}
