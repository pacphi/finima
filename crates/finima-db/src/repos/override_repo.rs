use sqlx::PgPool;
use uuid::Uuid;

use finima_core::errors::AppError;
use finima_core::models::UserCategoryOverride;

/// PostgreSQL implementation of user category override repository operations.
#[derive(Clone)]
pub struct PgOverrideRepo {
    pool: PgPool,
}

impl PgOverrideRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create or update an override by (user_id, description_pattern).
    pub async fn create_or_update(
        &self,
        user_id: Uuid,
        description_pattern: &str,
        category: &str,
        subcategory: &str,
    ) -> Result<UserCategoryOverride, AppError> {
        let row = sqlx::query_as::<_, UserCategoryOverride>(
            r#"
            INSERT INTO user_category_overrides (id, user_id, description_pattern, category, subcategory)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (user_id, description_pattern) DO UPDATE
            SET category = EXCLUDED.category,
                subcategory = EXCLUDED.subcategory
            RETURNING id, user_id, description_pattern, category, subcategory
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(user_id)
        .bind(description_pattern)
        .bind(category)
        .bind(subcategory)
        .fetch_one(&self.pool)
        .await?;

        Ok(row)
    }

    /// List all overrides for a user.
    pub async fn list_by_user(&self, user_id: Uuid) -> Result<Vec<UserCategoryOverride>, AppError> {
        let rows = sqlx::query_as::<_, UserCategoryOverride>(
            r#"
            SELECT id, user_id, description_pattern, category, subcategory
            FROM user_category_overrides
            WHERE user_id = $1
            ORDER BY description_pattern
            "#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    /// Find the first override whose pattern matches the given description (case-insensitive).
    pub async fn find_matching(
        &self,
        user_id: Uuid,
        description: &str,
    ) -> Result<Option<UserCategoryOverride>, AppError> {
        let row = sqlx::query_as::<_, UserCategoryOverride>(
            r#"
            SELECT id, user_id, description_pattern, category, subcategory
            FROM user_category_overrides
            WHERE user_id = $1
              AND LOWER($2) LIKE '%' || LOWER(description_pattern) || '%'
            LIMIT 1
            "#,
        )
        .bind(user_id)
        .bind(description)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row)
    }
}
