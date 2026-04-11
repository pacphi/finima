use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Budget {
    pub id: Uuid,
    pub portfolio_id: Uuid,
    pub category: String,
    pub monthly_limit: Decimal,
    pub rollover: bool,
    pub month: NaiveDate,
}
