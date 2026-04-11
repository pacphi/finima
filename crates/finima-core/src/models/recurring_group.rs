use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::types::Frequency;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct RecurringGroup {
    pub id: Uuid,
    pub portfolio_id: Uuid,
    pub merchant_name: String,
    pub category: String,
    pub frequency: Frequency,
    pub avg_amount: Decimal,
    pub is_confirmed: bool,
    pub next_expected_date: Option<NaiveDate>,
    pub metadata: serde_json::Value,
}
