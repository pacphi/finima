use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct UserCategoryOverride {
    pub id: Uuid,
    pub user_id: Uuid,
    pub description_pattern: String,
    pub category: String,
    pub subcategory: String,
}
