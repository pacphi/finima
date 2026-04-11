use chrono::NaiveDate;
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use finima_core::errors::AppError;
use finima_core::models::SavingsGoal;

/// PostgreSQL implementation of savings goal repository operations.
#[derive(Clone)]
pub struct PgSavingsGoalRepo {
    pool: PgPool,
}

impl PgSavingsGoalRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create a new savings goal.
    pub async fn create(
        &self,
        portfolio_id: Uuid,
        name: &str,
        target_amount: Decimal,
        target_date: Option<NaiveDate>,
        linked_account_id: Option<Uuid>,
    ) -> Result<SavingsGoal, AppError> {
        let goal = sqlx::query_as::<_, SavingsGoal>(
            r#"
            INSERT INTO savings_goals (id, portfolio_id, name, target_amount, current_amount,
                                       target_date, linked_account_id)
            VALUES ($1, $2, $3, $4, 0, $5, $6)
            RETURNING id, portfolio_id, name, target_amount, current_amount,
                      target_date, linked_account_id
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(portfolio_id)
        .bind(name)
        .bind(target_amount)
        .bind(target_date)
        .bind(linked_account_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(goal)
    }

    /// List all savings goals for a portfolio.
    pub async fn list_by_portfolio(
        &self,
        portfolio_id: Uuid,
    ) -> Result<Vec<SavingsGoal>, AppError> {
        let goals = sqlx::query_as::<_, SavingsGoal>(
            r#"
            SELECT id, portfolio_id, name, target_amount, current_amount,
                   target_date, linked_account_id
            FROM savings_goals
            WHERE portfolio_id = $1
            ORDER BY name
            "#,
        )
        .bind(portfolio_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(goals)
    }

    /// Update a savings goal. Only non-None fields are applied.
    pub async fn update(
        &self,
        id: Uuid,
        name: Option<&str>,
        target_amount: Option<Decimal>,
        current_amount: Option<Decimal>,
        target_date: Option<NaiveDate>,
    ) -> Result<SavingsGoal, AppError> {
        let goal = sqlx::query_as::<_, SavingsGoal>(
            r#"
            UPDATE savings_goals
            SET name = COALESCE($2, name),
                target_amount = COALESCE($3, target_amount),
                current_amount = COALESCE($4, current_amount),
                target_date = COALESCE($5, target_date)
            WHERE id = $1
            RETURNING id, portfolio_id, name, target_amount, current_amount,
                      target_date, linked_account_id
            "#,
        )
        .bind(id)
        .bind(name)
        .bind(target_amount)
        .bind(current_amount)
        .bind(target_date)
        .fetch_one(&self.pool)
        .await?;

        Ok(goal)
    }

    /// Delete a savings goal by ID.
    pub async fn delete(&self, id: Uuid) -> Result<(), AppError> {
        let result = sqlx::query("DELETE FROM savings_goals WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore] // Requires a running PostgreSQL database
    fn test_savings_goal_repo() {
        // Integration test placeholder
    }
}
