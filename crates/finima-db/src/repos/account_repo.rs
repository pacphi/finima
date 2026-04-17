use async_trait::async_trait;
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use finima_core::errors::AppError;
use finima_core::models::Account;
use finima_core::traits::AccountRepo;

/// PostgreSQL implementation of AccountRepo.
#[derive(Clone)]
pub struct PgAccountRepo {
    pool: PgPool,
}

impl PgAccountRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AccountRepo for PgAccountRepo {
    async fn create(&self, account: &Account) -> Result<Account, AppError> {
        let result = sqlx::query_as::<_, Account>(
            r#"
            INSERT INTO accounts (id, portfolio_id, name, institution, account_type, currency,
                                  opening_balance, is_primary_income, is_archived, notes, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, NOW())
            RETURNING id, portfolio_id, name, institution, account_type, currency,
                      opening_balance, is_primary_income, is_archived, notes, created_at,
                      sign_convention_override
            "#,
        )
        .bind(account.id)
        .bind(account.portfolio_id)
        .bind(&account.name)
        .bind(&account.institution)
        .bind(account.account_type.to_string())
        .bind(&account.currency)
        .bind(account.opening_balance)
        .bind(account.is_primary_income)
        .bind(account.is_archived)
        .bind(&account.notes)
        .fetch_one(&self.pool)
        .await?;

        Ok(result)
    }

    async fn list_by_portfolio(&self, portfolio_id: Uuid) -> Result<Vec<Account>, AppError> {
        let accounts = sqlx::query_as::<_, Account>(
            r#"
            SELECT id, portfolio_id, name, institution, account_type, currency,
                   opening_balance, is_primary_income, is_archived, notes, created_at,
                   sign_convention_override
            FROM accounts
            WHERE portfolio_id = $1 AND is_archived = false
            ORDER BY created_at
            "#,
        )
        .bind(portfolio_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(accounts)
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Account, AppError> {
        let account = sqlx::query_as::<_, Account>(
            r#"
            SELECT id, portfolio_id, name, institution, account_type, currency,
                   opening_balance, is_primary_income, is_archived, notes, created_at,
                   sign_convention_override
            FROM accounts
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await?;

        Ok(account)
    }

    async fn update(&self, account: &Account) -> Result<Account, AppError> {
        let result = sqlx::query_as::<_, Account>(
            r#"
            UPDATE accounts
            SET portfolio_id = $2, name = $3, institution = $4, account_type = $5,
                currency = $6, opening_balance = $7, is_primary_income = $8, notes = $9
            WHERE id = $1
            RETURNING id, portfolio_id, name, institution, account_type, currency,
                      opening_balance, is_primary_income, is_archived, notes, created_at,
                      sign_convention_override
            "#,
        )
        .bind(account.id)
        .bind(account.portfolio_id)
        .bind(&account.name)
        .bind(&account.institution)
        .bind(account.account_type.to_string())
        .bind(&account.currency)
        .bind(account.opening_balance)
        .bind(account.is_primary_income)
        .bind(&account.notes)
        .fetch_one(&self.pool)
        .await?;

        Ok(result)
    }

    async fn archive(&self, id: Uuid) -> Result<(), AppError> {
        sqlx::query("UPDATE accounts SET is_archived = true WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn delete(&self, id: Uuid) -> Result<(), AppError> {
        sqlx::query("DELETE FROM accounts WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn set_primary_income(&self, id: Uuid, is_primary: bool) -> Result<(), AppError> {
        sqlx::query("UPDATE accounts SET is_primary_income = $2 WHERE id = $1")
            .bind(id)
            .bind(is_primary)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn set_sign_convention_override(
        &self,
        id: Uuid,
        convention: Option<finima_core::services::sign_normalizer::SignConvention>,
    ) -> Result<(), AppError> {
        // Persist as TEXT for clarity in the DB; sqlx::Type derive on
        // SignConvention matches the column convention.
        sqlx::query("UPDATE accounts SET sign_convention_override = $2 WHERE id = $1")
            .bind(id)
            .bind(convention)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn compute_balance(&self, id: Uuid) -> Result<Decimal, AppError> {
        let balance = sqlx::query_scalar::<_, Decimal>(
            r#"
            SELECT a.opening_balance + COALESCE(SUM(t.amount), 0)
            FROM accounts a
            LEFT JOIN transactions t ON t.account_id = a.id
            WHERE a.id = $1
            GROUP BY a.id, a.opening_balance
            "#,
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await?;

        Ok(balance)
    }
}
