use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
}
