use async_trait::async_trait;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::errors::AppError;
use crate::models::{Account, Portfolio, User};

/// Repository trait for User operations.
#[async_trait]
pub trait UserRepo: Send + Sync {
    async fn create_user(&self, email: &str, display_name: &str) -> Result<User, AppError>;
    async fn find_by_email(&self, email: &str) -> Result<Option<User>, AppError>;
    async fn find_by_id(&self, id: Uuid) -> Result<User, AppError>;
    async fn update_preferences(
        &self,
        id: Uuid,
        preferences: serde_json::Value,
    ) -> Result<User, AppError>;
}

/// Repository trait for Portfolio operations.
#[async_trait]
pub trait PortfolioRepo: Send + Sync {
    async fn create(&self, user_id: Uuid, name: &str) -> Result<Portfolio, AppError>;
    async fn list_by_user(&self, user_id: Uuid) -> Result<Vec<Portfolio>, AppError>;
    async fn find_by_id(&self, id: Uuid) -> Result<Portfolio, AppError>;
    async fn update(&self, id: Uuid, name: &str) -> Result<Portfolio, AppError>;
    /// Verify that the given user owns the portfolio. Returns an error if not.
    async fn verify_ownership(&self, portfolio_id: Uuid, user_id: Uuid) -> Result<(), AppError>;
}

/// Repository trait for Account operations.
#[async_trait]
pub trait AccountRepo: Send + Sync {
    async fn create(&self, account: &Account) -> Result<Account, AppError>;
    async fn list_by_portfolio(&self, portfolio_id: Uuid) -> Result<Vec<Account>, AppError>;
    async fn find_by_id(&self, id: Uuid) -> Result<Account, AppError>;
    async fn update(&self, account: &Account) -> Result<Account, AppError>;
    async fn archive(&self, id: Uuid) -> Result<(), AppError>;
    /// Hard-delete an account and all dependent rows via ON DELETE CASCADE
    /// (transactions, uploads, account_flows, flow_patterns). Dangerous —
    /// cannot be undone. Caller is responsible for removing any related
    /// object-storage artifacts (e.g. S3 upload files) before invoking.
    async fn delete(&self, id: Uuid) -> Result<(), AppError>;
    async fn set_primary_income(&self, id: Uuid, is_primary: bool) -> Result<(), AppError>;
    /// Set or clear the per-account sign-convention override.
    /// When `convention` is `None`, the override is removed and the
    /// SignNormalizer falls back to the institution YAML rule, then
    /// autodetection, then the account-type default. See ADR-018.
    async fn set_sign_convention_override(
        &self,
        id: Uuid,
        convention: Option<crate::services::sign_normalizer::SignConvention>,
    ) -> Result<(), AppError>;
    /// Compute current balance: opening_balance + SUM(transactions.amount).
    async fn compute_balance(&self, id: Uuid) -> Result<Decimal, AppError>;
}
