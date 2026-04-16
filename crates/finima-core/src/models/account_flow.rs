use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AccountFlow {
    pub id: Uuid,
    pub portfolio_id: Uuid,
    pub source_account_id: Uuid,
    #[serde(rename = "destination_account_id")]
    pub target_account_id: Uuid,
    pub source_transaction_id: Option<Uuid>,
    #[serde(rename = "destination_transaction_id")]
    pub target_transaction_id: Option<Uuid>,
    pub amount: Decimal,
    #[serde(rename = "date")]
    pub flow_date: NaiveDate,
    pub is_auto_detected: bool,
    pub is_confirmed: bool,
    pub flow_group_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}
