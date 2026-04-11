use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct FlowGroup {
    pub id: Uuid,
    pub portfolio_id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
}
