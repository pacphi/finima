use chrono::NaiveDate;
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use finima_core::errors::AppError;
use finima_core::models::AccountFlow;

/// Input for creating a new account flow.
#[derive(Debug, Clone)]
pub struct NewAccountFlow {
    pub portfolio_id: Uuid,
    pub source_account_id: Uuid,
    pub target_account_id: Uuid,
    pub source_transaction_id: Option<Uuid>,
    pub target_transaction_id: Option<Uuid>,
    pub amount: Decimal,
    pub flow_date: NaiveDate,
    pub is_auto_detected: bool,
}

/// PostgreSQL implementation of account flow repository operations.
#[derive(Clone)]
pub struct PgFlowRepo {
    pool: PgPool,
}

impl PgFlowRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create a new account flow record.
    pub async fn create(&self, flow: &NewAccountFlow) -> Result<AccountFlow, AppError> {
        let result = sqlx::query_as::<_, AccountFlow>(
            r#"
            INSERT INTO account_flows (
                id, portfolio_id, source_account_id, target_account_id,
                source_transaction_id, target_transaction_id,
                amount, flow_date, is_auto_detected, is_confirmed, flow_group_id, created_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, false, NULL, NOW())
            RETURNING id, portfolio_id, source_account_id, target_account_id,
                      source_transaction_id, target_transaction_id,
                      amount, flow_date, is_auto_detected, is_confirmed,
                      flow_group_id, created_at
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(flow.portfolio_id)
        .bind(flow.source_account_id)
        .bind(flow.target_account_id)
        .bind(flow.source_transaction_id)
        .bind(flow.target_transaction_id)
        .bind(flow.amount)
        .bind(flow.flow_date)
        .bind(flow.is_auto_detected)
        .fetch_one(&self.pool)
        .await?;

        Ok(result)
    }

    /// List all flows for a portfolio in a given month.
    pub async fn list_by_portfolio_month(
        &self,
        portfolio_id: Uuid,
        month: NaiveDate,
    ) -> Result<Vec<AccountFlow>, AppError> {
        let flows = sqlx::query_as::<_, AccountFlow>(
            r#"
            SELECT id, portfolio_id, source_account_id, target_account_id,
                   source_transaction_id, target_transaction_id,
                   amount, flow_date, is_auto_detected, is_confirmed,
                   flow_group_id, created_at
            FROM account_flows
            WHERE portfolio_id = $1
              AND date_trunc('month', flow_date) = date_trunc('month', $2::date)
            ORDER BY flow_date, amount DESC
            "#,
        )
        .bind(portfolio_id)
        .bind(month)
        .fetch_all(&self.pool)
        .await?;

        Ok(flows)
    }

    /// Find a single flow by ID.
    pub async fn find_by_id(&self, id: Uuid) -> Result<AccountFlow, AppError> {
        let flow = sqlx::query_as::<_, AccountFlow>(
            r#"
            SELECT id, portfolio_id, source_account_id, target_account_id,
                   source_transaction_id, target_transaction_id,
                   amount, flow_date, is_auto_detected, is_confirmed,
                   flow_group_id, created_at
            FROM account_flows
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await?;

        Ok(flow)
    }

    /// Confirm a flow (mark as user-verified).
    pub async fn confirm(&self, id: Uuid) -> Result<(), AppError> {
        sqlx::query("UPDATE account_flows SET is_confirmed = true WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Dismiss a flow by deleting it.
    pub async fn dismiss(&self, id: Uuid) -> Result<(), AppError> {
        self.delete(id).await
    }

    /// Delete a flow by ID.
    pub async fn delete(&self, id: Uuid) -> Result<(), AppError> {
        let result = sqlx::query("DELETE FROM account_flows WHERE id = $1")
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
    fn test_flow_repo() {
        // Integration test placeholder
    }
}
