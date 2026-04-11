use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SavingsGoal {
    pub id: Uuid,
    pub portfolio_id: Uuid,
    pub name: String,
    pub target_amount: Decimal,
    pub current_amount: Decimal,
    pub target_date: Option<NaiveDate>,
    pub linked_account_id: Option<Uuid>,
}
