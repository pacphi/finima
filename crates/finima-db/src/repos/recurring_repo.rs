use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

use finima_core::errors::AppError;
use finima_core::models::RecurringGroup;
use finima_core::types::Frequency;

/// PostgreSQL implementation of recurring group repository operations.
#[derive(Clone)]
pub struct PgRecurringRepo {
    pool: PgPool,
}

/// Input for upserting a recurring group candidate.
#[derive(Debug, Clone)]
pub struct RecurringGroupInsert {
    pub merchant_name: String,
    pub category: String,
    pub frequency: Frequency,
    pub avg_amount: Decimal,
    pub next_expected_date: Option<NaiveDate>,
    pub metadata: serde_json::Value,
}

/// Updateable fields for a recurring group.
#[derive(Debug, Clone, Deserialize)]
pub struct RecurringGroupUpdate {
    pub merchant_name: Option<String>,
    pub category: Option<String>,
    pub frequency: Option<Frequency>,
    pub avg_amount: Option<Decimal>,
    pub next_expected_date: Option<NaiveDate>,
}

impl PgRecurringRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Insert or update a recurring group by portfolio + merchant_name.
    pub async fn upsert(
        &self,
        portfolio_id: Uuid,
        candidate: RecurringGroupInsert,
    ) -> Result<RecurringGroup, AppError> {
        let group = sqlx::query_as::<_, RecurringGroup>(
            r#"
            INSERT INTO recurring_groups (
                id, portfolio_id, merchant_name, category, frequency,
                avg_amount, is_confirmed, next_expected_date, metadata
            )
            VALUES ($1, $2, $3, $4, $5, $6, false, $7, $8)
            ON CONFLICT (portfolio_id, merchant_name) DO UPDATE
            SET category = EXCLUDED.category,
                frequency = EXCLUDED.frequency,
                avg_amount = EXCLUDED.avg_amount,
                next_expected_date = EXCLUDED.next_expected_date,
                metadata = EXCLUDED.metadata
            RETURNING id, portfolio_id, merchant_name, category, frequency,
                      avg_amount, is_confirmed, next_expected_date, metadata
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(portfolio_id)
        .bind(&candidate.merchant_name)
        .bind(&candidate.category)
        .bind(candidate.frequency.to_string())
        .bind(candidate.avg_amount)
        .bind(candidate.next_expected_date)
        .bind(&candidate.metadata)
        .fetch_one(&self.pool)
        .await?;

        Ok(group)
    }

    /// Delete all unconfirmed recurring groups for a portfolio.
    ///
    /// Used at the start of a fresh detection pass so candidates that no
    /// longer satisfy the detector's thresholds (e.g. a "variable" group
    /// that has dropped below the minimum occurrence count) actually
    /// disappear from the UI. User-confirmed groups are preserved.
    pub async fn delete_unconfirmed_by_portfolio(
        &self,
        portfolio_id: Uuid,
    ) -> Result<u64, AppError> {
        let result = sqlx::query(
            r#"
            DELETE FROM recurring_groups
            WHERE portfolio_id = $1
              AND is_confirmed = false
            "#,
        )
        .bind(portfolio_id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// List all non-dismissed recurring groups for a portfolio.
    pub async fn list_by_portfolio(
        &self,
        portfolio_id: Uuid,
    ) -> Result<Vec<RecurringGroup>, AppError> {
        let groups = sqlx::query_as::<_, RecurringGroup>(
            r#"
            SELECT id, portfolio_id, merchant_name, category, frequency,
                   avg_amount, is_confirmed, next_expected_date, metadata
            FROM recurring_groups
            WHERE portfolio_id = $1
              AND (metadata->>'dismissed')::boolean IS NOT TRUE
            ORDER BY ABS(avg_amount) DESC
            "#,
        )
        .bind(portfolio_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(groups)
    }

    /// Confirm a recurring group.
    pub async fn confirm(&self, id: Uuid) -> Result<(), AppError> {
        sqlx::query("UPDATE recurring_groups SET is_confirmed = true WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Dismiss a recurring group (soft delete via metadata flag).
    pub async fn dismiss(&self, id: Uuid) -> Result<(), AppError> {
        sqlx::query(
            r#"
            UPDATE recurring_groups
            SET metadata = jsonb_set(COALESCE(metadata, '{}'::jsonb), '{dismissed}', 'true')
            WHERE id = $1
            "#,
        )
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Update editable fields on a recurring group.
    pub async fn update(
        &self,
        id: Uuid,
        fields: &RecurringGroupUpdate,
    ) -> Result<RecurringGroup, AppError> {
        let group = sqlx::query_as::<_, RecurringGroup>(
            r#"
            UPDATE recurring_groups
            SET merchant_name = COALESCE($2, merchant_name),
                category = COALESCE($3, category),
                frequency = COALESCE($4, frequency),
                avg_amount = COALESCE($5, avg_amount),
                next_expected_date = COALESCE($6, next_expected_date)
            WHERE id = $1
            RETURNING id, portfolio_id, merchant_name, category, frequency,
                      avg_amount, is_confirmed, next_expected_date, metadata
            "#,
        )
        .bind(id)
        .bind(&fields.merchant_name)
        .bind(&fields.category)
        .bind(fields.frequency.map(|f| f.to_string()))
        .bind(fields.avg_amount)
        .bind(fields.next_expected_date)
        .fetch_one(&self.pool)
        .await?;

        Ok(group)
    }
}
