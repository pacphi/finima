use sqlx::PgPool;
use uuid::Uuid;

use finima_core::errors::AppError;
use finima_core::models::FlowGroup;

/// PostgreSQL implementation of flow group repository operations.
#[derive(Clone)]
pub struct PgFlowGroupRepo {
    pool: PgPool,
}

impl PgFlowGroupRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create a new flow group.
    pub async fn create(&self, portfolio_id: Uuid, name: &str) -> Result<FlowGroup, AppError> {
        let group = sqlx::query_as::<_, FlowGroup>(
            r#"
            INSERT INTO flow_groups (id, portfolio_id, name, created_at)
            VALUES ($1, $2, $3, NOW())
            RETURNING id, portfolio_id, name, created_at
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(portfolio_id)
        .bind(name)
        .fetch_one(&self.pool)
        .await?;

        Ok(group)
    }

    /// List all flow groups for a portfolio.
    pub async fn list_by_portfolio(&self, portfolio_id: Uuid) -> Result<Vec<FlowGroup>, AppError> {
        let groups = sqlx::query_as::<_, FlowGroup>(
            r#"
            SELECT id, portfolio_id, name, created_at
            FROM flow_groups
            WHERE portfolio_id = $1
            ORDER BY name
            "#,
        )
        .bind(portfolio_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(groups)
    }

    /// Update a flow group name.
    pub async fn update(&self, id: Uuid, name: &str) -> Result<FlowGroup, AppError> {
        let group = sqlx::query_as::<_, FlowGroup>(
            r#"
            UPDATE flow_groups
            SET name = $2
            WHERE id = $1
            RETURNING id, portfolio_id, name, created_at
            "#,
        )
        .bind(id)
        .bind(name)
        .fetch_one(&self.pool)
        .await?;

        Ok(group)
    }

    /// Delete a flow group by ID. Also unlinks any flows assigned to it.
    pub async fn delete(&self, id: Uuid) -> Result<(), AppError> {
        // First, unlink any flows that reference this group.
        sqlx::query("UPDATE account_flows SET flow_group_id = NULL WHERE flow_group_id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;

        let result = sqlx::query("DELETE FROM flow_groups WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound);
        }

        Ok(())
    }

    /// Assign a flow to a group.
    pub async fn assign_flow(&self, flow_id: Uuid, group_id: Uuid) -> Result<(), AppError> {
        let result = sqlx::query("UPDATE account_flows SET flow_group_id = $2 WHERE id = $1")
            .bind(flow_id)
            .bind(group_id)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound);
        }

        Ok(())
    }

    /// Remove a flow from its group (set flow_group_id to NULL).
    pub async fn remove_flow(&self, flow_id: Uuid) -> Result<(), AppError> {
        sqlx::query("UPDATE account_flows SET flow_group_id = NULL WHERE id = $1")
            .bind(flow_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}
