use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::services::sign_normalizer::SignConvention;
use crate::types::AccountType;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Account {
    pub id: Uuid,
    pub portfolio_id: Uuid,
    pub name: String,
    pub institution: Option<String>,
    pub account_type: AccountType,
    pub currency: String,
    pub opening_balance: Decimal,
    pub is_primary_income: bool,
    pub is_archived: bool,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    /// Per-account user-set sign-convention override. When non-NULL,
    /// takes precedence over institution YAML rules and autodetection
    /// in the SignNormalizer chain. Set via the "Flip this account"
    /// button on the Account detail page. See ADR-018.
    pub sign_convention_override: Option<SignConvention>,
}
