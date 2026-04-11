use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Transaction {
    pub id: Uuid,
    pub account_id: Uuid,
    pub date: NaiveDate,
    pub amount: Decimal,
    pub description: String,
    pub original_description: String,
    pub category: Option<String>,
    pub subcategory: Option<String>,
    pub merchant_name: Option<String>,
    pub tags: Vec<String>,
    pub notes: Option<String>,
    pub is_recurring: bool,
    pub recurring_group_id: Option<Uuid>,
    pub llm_confidence: Option<f64>,
    pub user_overridden: bool,
    pub dedup_hash: String,
    pub created_at: DateTime<Utc>,
}
