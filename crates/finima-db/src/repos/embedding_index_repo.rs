use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

/// Insert payload for a new embedding_index row.
#[derive(Debug, Clone)]
pub struct NewEmbeddingIndex {
    pub portfolio_id: Uuid,
    pub description: String,
    pub description_normalized: String,
    pub embedding: Option<Vec<u8>>,
    pub embedding_dim: Option<i32>,
    pub category: String,
    pub subcategory: String,
    pub confidence: f64,
    pub source_tier: String,
}

/// Read model for an embedding_index row.
#[derive(Debug, Clone, FromRow)]
pub struct EmbeddingIndexRow {
    pub id: Uuid,
    pub portfolio_id: Uuid,
    pub description: String,
    pub description_normalized: String,
    pub embedding: Option<Vec<u8>>,
    pub embedding_dim: Option<i32>,
    pub category: String,
    pub subcategory: String,
    pub confidence: f64,
    pub source_tier: String,
    pub created_at: DateTime<Utc>,
}

/// Repository for the embedding_index table (Tier 2 ruvector persistence).
#[derive(Clone)]
pub struct EmbeddingIndexRepo {
    pool: PgPool,
}

impl EmbeddingIndexRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Insert a new row and return its generated id.
    pub async fn insert(&self, row: NewEmbeddingIndex) -> Result<Uuid, sqlx::Error> {
        let id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO embedding_index (
                id, portfolio_id, description, description_normalized,
                embedding, embedding_dim, category, subcategory,
                confidence, source_tier, created_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, NOW())
            "#,
        )
        .bind(id)
        .bind(row.portfolio_id)
        .bind(&row.description)
        .bind(&row.description_normalized)
        .bind(row.embedding.as_deref())
        .bind(row.embedding_dim)
        .bind(&row.category)
        .bind(&row.subcategory)
        .bind(row.confidence)
        .bind(&row.source_tier)
        .execute(&self.pool)
        .await?;

        Ok(id)
    }

    /// List all embedding rows for a given portfolio ordered by created_at ascending.
    pub async fn list_for_portfolio(
        &self,
        portfolio_id: Uuid,
    ) -> Result<Vec<EmbeddingIndexRow>, sqlx::Error> {
        let rows = sqlx::query_as::<_, EmbeddingIndexRow>(
            r#"
            SELECT id, portfolio_id, description, description_normalized,
                   embedding, embedding_dim, category, subcategory,
                   confidence, source_tier, created_at
            FROM embedding_index
            WHERE portfolio_id = $1
            ORDER BY created_at ASC
            "#,
        )
        .bind(portfolio_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    /// Count rows for a given portfolio.
    pub async fn count_for_portfolio(&self, portfolio_id: Uuid) -> Result<i64, sqlx::Error> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*)::BIGINT FROM embedding_index WHERE portfolio_id = $1",
        )
        .bind(portfolio_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(count)
    }

    /// Delete all rows for a given portfolio. Returns the number of rows removed.
    pub async fn delete_for_portfolio(&self, portfolio_id: Uuid) -> Result<u64, sqlx::Error> {
        let result = sqlx::query("DELETE FROM embedding_index WHERE portfolio_id = $1")
            .bind(portfolio_id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected())
    }
}
