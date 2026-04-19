use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

/// Insert payload for a new flow_patterns row.
#[derive(Debug, Clone)]
pub struct NewFlowPattern {
    pub portfolio_id: Uuid,
    pub description_text: String,
    pub source_account_id: Uuid,
    pub target_account_id: Uuid,
    pub confidence: f64,
    pub embedding: Option<Vec<u8>>,
    pub embedding_dim: Option<i32>,
}

/// Read model for a flow_patterns row.
#[derive(Debug, Clone, FromRow)]
pub struct FlowPatternRow {
    pub id: Uuid,
    pub portfolio_id: Uuid,
    pub description_text: String,
    pub source_account_id: Uuid,
    pub target_account_id: Uuid,
    pub confidence: f64,
    pub match_count: i32,
    pub embedding: Option<Vec<u8>>,
    pub embedding_dim: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Repository for the flow_patterns table (SONA-enhanced flow detection persistence).
#[derive(Clone)]
pub struct FlowPatternRepo {
    pool: PgPool,
}

impl FlowPatternRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Insert a new row and return its generated id. Sets match_count=1 and
    /// created_at/updated_at=NOW().
    pub async fn insert(&self, row: NewFlowPattern) -> Result<Uuid, sqlx::Error> {
        let id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO flow_patterns (
                id, portfolio_id, description_text,
                source_account_id, target_account_id,
                confidence, match_count,
                description_embedding, embedding_dim,
                created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, 1, $7, $8, NOW(), NOW())
            "#,
        )
        .bind(id)
        .bind(row.portfolio_id)
        .bind(&row.description_text)
        .bind(row.source_account_id)
        .bind(row.target_account_id)
        .bind(row.confidence)
        .bind(row.embedding.as_deref())
        .bind(row.embedding_dim)
        .execute(&self.pool)
        .await?;

        Ok(id)
    }

    /// Upsert a confirmed pattern keyed on
    /// (portfolio_id, source_account_id, target_account_id, description_text).
    ///
    /// If a matching row exists: increment match_count, bump updated_at=NOW(),
    /// keep the higher of old/new confidence, and leave the existing embedding
    /// in place if the incoming one is None.
    ///
    /// If no row exists: insert fresh. Returns the row's UUID in both cases.
    pub async fn upsert_confirmed(&self, row: NewFlowPattern) -> Result<Uuid, sqlx::Error> {
        let mut tx = self.pool.begin().await?;

        let existing: Option<(Uuid,)> = sqlx::query_as(
            r#"
            SELECT id
            FROM flow_patterns
            WHERE portfolio_id = $1
              AND source_account_id = $2
              AND target_account_id = $3
              AND description_text = $4
            FOR UPDATE
            "#,
        )
        .bind(row.portfolio_id)
        .bind(row.source_account_id)
        .bind(row.target_account_id)
        .bind(&row.description_text)
        .fetch_optional(&mut *tx)
        .await?;

        let id = if let Some((existing_id,)) = existing {
            if row.embedding.is_some() {
                sqlx::query(
                    r#"
                    UPDATE flow_patterns
                    SET match_count = match_count + 1,
                        updated_at = NOW(),
                        confidence = GREATEST(confidence, $2),
                        description_embedding = $3,
                        embedding_dim = $4
                    WHERE id = $1
                    "#,
                )
                .bind(existing_id)
                .bind(row.confidence)
                .bind(row.embedding.as_deref())
                .bind(row.embedding_dim)
                .execute(&mut *tx)
                .await?;
            } else {
                sqlx::query(
                    r#"
                    UPDATE flow_patterns
                    SET match_count = match_count + 1,
                        updated_at = NOW(),
                        confidence = GREATEST(confidence, $2)
                    WHERE id = $1
                    "#,
                )
                .bind(existing_id)
                .bind(row.confidence)
                .execute(&mut *tx)
                .await?;
            }
            existing_id
        } else {
            let new_id = Uuid::new_v4();
            sqlx::query(
                r#"
                INSERT INTO flow_patterns (
                    id, portfolio_id, description_text,
                    source_account_id, target_account_id,
                    confidence, match_count,
                    description_embedding, embedding_dim,
                    created_at, updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, 1, $7, $8, NOW(), NOW())
                "#,
            )
            .bind(new_id)
            .bind(row.portfolio_id)
            .bind(&row.description_text)
            .bind(row.source_account_id)
            .bind(row.target_account_id)
            .bind(row.confidence)
            .bind(row.embedding.as_deref())
            .bind(row.embedding_dim)
            .execute(&mut *tx)
            .await?;
            new_id
        };

        tx.commit().await?;
        Ok(id)
    }

    /// List all flow patterns for a given portfolio ordered by updated_at DESC.
    pub async fn list_for_portfolio(
        &self,
        portfolio_id: Uuid,
    ) -> Result<Vec<FlowPatternRow>, sqlx::Error> {
        let rows = sqlx::query_as::<_, FlowPatternRow>(
            r#"
            SELECT id, portfolio_id, description_text,
                   source_account_id, target_account_id,
                   confidence, match_count,
                   description_embedding AS embedding, embedding_dim,
                   created_at, updated_at
            FROM flow_patterns
            WHERE portfolio_id = $1
            ORDER BY updated_at DESC
            "#,
        )
        .bind(portfolio_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    /// List all flow patterns anchored on a given source account.
    pub async fn list_for_source(
        &self,
        source_account_id: Uuid,
    ) -> Result<Vec<FlowPatternRow>, sqlx::Error> {
        let rows = sqlx::query_as::<_, FlowPatternRow>(
            r#"
            SELECT id, portfolio_id, description_text,
                   source_account_id, target_account_id,
                   confidence, match_count,
                   description_embedding AS embedding, embedding_dim,
                   created_at, updated_at
            FROM flow_patterns
            WHERE source_account_id = $1
            ORDER BY updated_at DESC
            "#,
        )
        .bind(source_account_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    /// Count rows for a given portfolio.
    pub async fn count_for_portfolio(&self, portfolio_id: Uuid) -> Result<i64, sqlx::Error> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*)::BIGINT FROM flow_patterns WHERE portfolio_id = $1",
        )
        .bind(portfolio_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(count)
    }

    /// Delete all rows for a given portfolio. Returns the number of rows removed.
    pub async fn delete_for_portfolio(&self, portfolio_id: Uuid) -> Result<u64, sqlx::Error> {
        let result = sqlx::query("DELETE FROM flow_patterns WHERE portfolio_id = $1")
            .bind(portfolio_id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected())
    }

    /// Record a dismissal by halving the confidence of matching patterns
    /// (half-life decay, floored at 0.0). Returns rows affected.
    pub async fn record_dismissal(
        &self,
        portfolio_id: Uuid,
        source_account_id: Uuid,
        description_text: &str,
    ) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            r#"
            UPDATE flow_patterns
            SET confidence = GREATEST(confidence * 0.5, 0.0),
                updated_at = NOW()
            WHERE portfolio_id = $1
              AND source_account_id = $2
              AND description_text = $3
            "#,
        )
        .bind(portfolio_id)
        .bind(source_account_id)
        .bind(description_text)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }
}
