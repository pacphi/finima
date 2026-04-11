use chrono::NaiveDate;
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use finima_core::errors::AppError;
use finima_core::models::Budget;

/// PostgreSQL implementation of budget repository operations.
#[derive(Clone)]
pub struct PgBudgetRepo {
    pool: PgPool,
}

impl PgBudgetRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create or update a budget entry for a given portfolio, category, and month.
    ///
    /// Uses ON CONFLICT on (portfolio_id, category, month) to upsert.
    pub async fn create_or_update(
        &self,
        portfolio_id: Uuid,
        category: &str,
        monthly_limit: Decimal,
        rollover: bool,
        month: NaiveDate,
    ) -> Result<Budget, AppError> {
        let budget = sqlx::query_as::<_, Budget>(
            r#"
            INSERT INTO budgets (id, portfolio_id, category, monthly_limit, rollover, month)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (portfolio_id, category, month) DO UPDATE
            SET monthly_limit = EXCLUDED.monthly_limit,
                rollover = EXCLUDED.rollover
            RETURNING id, portfolio_id, category, monthly_limit, rollover, month
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(portfolio_id)
        .bind(category)
        .bind(monthly_limit)
        .bind(rollover)
        .bind(month)
        .fetch_one(&self.pool)
        .await?;

        Ok(budget)
    }

    /// List all budgets for a given portfolio and month.
    pub async fn list_by_portfolio_month(
        &self,
        portfolio_id: Uuid,
        month: NaiveDate,
    ) -> Result<Vec<Budget>, AppError> {
        let budgets = sqlx::query_as::<_, Budget>(
            r#"
            SELECT id, portfolio_id, category, monthly_limit, rollover, month
            FROM budgets
            WHERE portfolio_id = $1 AND month = $2
            ORDER BY category
            "#,
        )
        .bind(portfolio_id)
        .bind(month)
        .fetch_all(&self.pool)
        .await?;

        Ok(budgets)
    }

    /// Delete a budget by ID.
    pub async fn delete(&self, id: Uuid) -> Result<(), AppError> {
        let result = sqlx::query("DELETE FROM budgets WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound);
        }

        Ok(())
    }
}
