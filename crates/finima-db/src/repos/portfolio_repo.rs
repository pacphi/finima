use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use finima_core::errors::AppError;
use finima_core::models::Portfolio;
use finima_core::traits::PortfolioRepo;

/// PostgreSQL implementation of PortfolioRepo.
#[derive(Clone)]
pub struct PgPortfolioRepo {
    pool: PgPool,
}

impl PgPortfolioRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PortfolioRepo for PgPortfolioRepo {
    async fn create(&self, user_id: Uuid, name: &str) -> Result<Portfolio, AppError> {
        let portfolio = sqlx::query_as::<_, Portfolio>(
            r#"
            INSERT INTO portfolios (id, user_id, name, created_at)
            VALUES ($1, $2, $3, NOW())
            RETURNING id, user_id, name, created_at
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(user_id)
        .bind(name)
        .fetch_one(&self.pool)
        .await?;

        Ok(portfolio)
    }

    async fn list_by_user(&self, user_id: Uuid) -> Result<Vec<Portfolio>, AppError> {
        let portfolios = sqlx::query_as::<_, Portfolio>(
            "SELECT id, user_id, name, created_at FROM portfolios WHERE user_id = $1 ORDER BY created_at",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(portfolios)
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Portfolio, AppError> {
        let portfolio = sqlx::query_as::<_, Portfolio>(
            "SELECT id, user_id, name, created_at FROM portfolios WHERE id = $1",
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await?;

        Ok(portfolio)
    }

    async fn update(&self, id: Uuid, name: &str) -> Result<Portfolio, AppError> {
        let portfolio = sqlx::query_as::<_, Portfolio>(
            r#"
            UPDATE portfolios SET name = $2
            WHERE id = $1
            RETURNING id, user_id, name, created_at
            "#,
        )
        .bind(id)
        .bind(name)
        .fetch_one(&self.pool)
        .await?;

        Ok(portfolio)
    }

    async fn verify_ownership(&self, portfolio_id: Uuid, user_id: Uuid) -> Result<(), AppError> {
        let exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM portfolios WHERE id = $1 AND user_id = $2)",
        )
        .bind(portfolio_id)
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;

        if exists {
            Ok(())
        } else {
            Err(AppError::NotFound)
        }
    }
}

impl PgPortfolioRepo {
    /// Persist the Sona adapter state JSON for a portfolio.
    pub async fn save_sona_state(
        &self,
        portfolio_id: Uuid,
        state: &serde_json::Value,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE portfolios SET sona_state = $1, sona_state_updated_at = NOW() WHERE id = $2",
        )
        .bind(state)
        .bind(portfolio_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Load the Sona adapter state JSON for a portfolio, if any.
    pub async fn load_sona_state(
        &self,
        portfolio_id: Uuid,
    ) -> Result<Option<serde_json::Value>, sqlx::Error> {
        let state: Option<serde_json::Value> =
            sqlx::query_scalar("SELECT sona_state FROM portfolios WHERE id = $1")
                .bind(portfolio_id)
                .fetch_optional(&self.pool)
                .await?
                .flatten();

        Ok(state)
    }
}
