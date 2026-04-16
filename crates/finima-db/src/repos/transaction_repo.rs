use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use finima_core::errors::AppError;
use finima_core::models::Transaction;

/// A lightweight transaction struct for analysis functions (decoupled from
/// the full DB model). Mirrors `finima_analysis::TransactionForAnalysis`.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct TransactionForAnalysisRow {
    pub id: Uuid,
    pub date: NaiveDate,
    pub amount: Decimal,
    pub description: String,
    pub merchant_name: Option<String>,
    pub category: Option<String>,
    pub subcategory: Option<String>,
    pub account_id: Uuid,
    /// Canonical direction set by SignNormalizer at import time
    /// (see ADR-018). NULL means a legacy row not yet normalized;
    /// downstream Sankey logic excludes such rows.
    pub direction: Option<finima_core::TransactionDirection>,
}

/// PostgreSQL implementation of transaction repository operations.
#[derive(Clone)]
pub struct PgTransactionRepo {
    pool: PgPool,
}

/// A new transaction to be inserted (before it has a DB-generated id/created_at).
#[derive(Debug, Clone)]
pub struct NewTransaction {
    pub account_id: Uuid,
    pub date: NaiveDate,
    pub amount: Decimal,
    pub description: String,
    pub original_description: String,
    pub category: Option<String>,
    pub subcategory: Option<String>,
    pub merchant_name: Option<String>,
    pub memo: Option<String>,
    pub dedup_hash: String,
    /// Canonical direction (inflow/outflow) computed by the
    /// SignNormalizer at import time. See ADR-018.
    pub direction: finima_core::TransactionDirection,
}

/// Filters for listing transactions.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct TransactionFilters {
    pub account_id: Option<Uuid>,
    pub portfolio_id: Option<Uuid>,
    pub date_from: Option<NaiveDate>,
    pub date_to: Option<NaiveDate>,
    pub category: Option<String>,
    pub amount_min: Option<Decimal>,
    pub amount_max: Option<Decimal>,
    pub search_text: Option<String>,
}

/// Pagination parameters.
#[derive(Debug, Clone, Deserialize)]
pub struct Pagination {
    #[serde(default = "default_page")]
    pub page: i64,
    #[serde(default = "default_per_page")]
    pub per_page: i64,
}

fn default_page() -> i64 {
    1
}
fn default_per_page() -> i64 {
    50
}

impl Default for Pagination {
    fn default() -> Self {
        Self {
            page: 1,
            per_page: 50,
        }
    }
}

/// Sort parameters.
#[derive(Debug, Clone, Deserialize)]
pub struct Sort {
    #[serde(default = "default_sort_field")]
    pub field: String,
    #[serde(default = "default_sort_dir")]
    pub direction: String,
}

fn default_sort_field() -> String {
    "date".to_string()
}
fn default_sort_dir() -> String {
    "desc".to_string()
}

impl Default for Sort {
    fn default() -> Self {
        Self {
            field: "date".to_string(),
            direction: "desc".to_string(),
        }
    }
}

/// An update for LLM categorization results applied in bulk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmCategorizationUpdate {
    pub transaction_id: Uuid,
    pub category: String,
    pub subcategory: String,
    pub merchant_name: String,
    pub llm_confidence: f64,
}

impl PgTransactionRepo {
    /// Number of rows per SQL batch for bulk INSERT and UPDATE operations.
    ///
    /// Keeps the parameter count well within PostgreSQL's limits while
    /// avoiding excessive round-trips.
    const DB_CHUNK_SIZE: usize = 100;

    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Batch INSERT with ON CONFLICT (account_id, dedup_hash) DO NOTHING.
    /// Returns the count of actually inserted rows.
    pub async fn bulk_insert(
        &self,
        account_id: Uuid,
        transactions: &[NewTransaction],
    ) -> Result<usize, AppError> {
        if transactions.is_empty() {
            return Ok(0);
        }

        let mut inserted: usize = 0;

        // Process in chunks to avoid exceeding PostgreSQL parameter limits.
        for chunk in transactions.chunks(Self::DB_CHUNK_SIZE) {
            let mut ids = Vec::with_capacity(chunk.len());
            let mut account_ids = Vec::with_capacity(chunk.len());
            let mut dates = Vec::with_capacity(chunk.len());
            let mut amounts = Vec::with_capacity(chunk.len());
            let mut descriptions = Vec::with_capacity(chunk.len());
            let mut orig_descriptions = Vec::with_capacity(chunk.len());
            let mut categories: Vec<Option<String>> = Vec::with_capacity(chunk.len());
            let mut subcategories: Vec<Option<String>> = Vec::with_capacity(chunk.len());
            let mut merchant_names: Vec<Option<String>> = Vec::with_capacity(chunk.len());
            let mut dedup_hashes = Vec::with_capacity(chunk.len());
            let mut directions: Vec<String> = Vec::with_capacity(chunk.len());

            for txn in chunk {
                ids.push(Uuid::new_v4());
                account_ids.push(account_id);
                dates.push(txn.date);
                amounts.push(txn.amount);
                descriptions.push(txn.description.clone());
                orig_descriptions.push(txn.original_description.clone());
                categories.push(txn.category.clone());
                subcategories.push(txn.subcategory.clone());
                merchant_names.push(txn.merchant_name.clone());
                dedup_hashes.push(txn.dedup_hash.clone());
                directions.push(txn.direction.to_string());
            }

            let result = sqlx::query_scalar::<_, i64>(
                r#"
                WITH ins AS (
                    INSERT INTO transactions (
                        id, account_id, date, amount, description, original_description,
                        category, subcategory, merchant_name, dedup_hash, direction, created_at
                    )
                    SELECT * FROM UNNEST(
                        $1::uuid[], $2::uuid[], $3::date[], $4::numeric[],
                        $5::text[], $6::text[], $7::text[], $8::text[],
                        $9::text[], $10::text[], $11::text[]
                    ) AS t(id, account_id, date, amount, description, original_description,
                           category, subcategory, merchant_name, dedup_hash, direction)
                    CROSS JOIN (SELECT NOW() AS created_at) AS ts
                    ON CONFLICT (account_id, dedup_hash) DO NOTHING
                    RETURNING 1
                )
                SELECT COUNT(*) FROM ins
                "#,
            )
            .bind(&ids)
            .bind(&account_ids)
            .bind(&dates)
            .bind(&amounts)
            .bind(&descriptions)
            .bind(&orig_descriptions)
            .bind(&categories as &Vec<Option<String>>)
            .bind(&subcategories as &Vec<Option<String>>)
            .bind(&merchant_names as &Vec<Option<String>>)
            .bind(&dedup_hashes)
            .bind(&directions)
            .fetch_one(&self.pool)
            .await?;

            inserted += result as usize;
        }

        Ok(inserted)
    }

    /// Maximum allowed `per_page` / `limit` value. Any value above this is
    /// clamped to prevent oversized result sets regardless of the caller.
    const MAX_PER_PAGE: i64 = 100;

    /// Filtered, paginated, sorted query returning (rows, total_count).
    pub async fn list(
        &self,
        filters: &TransactionFilters,
        pagination: &Pagination,
        sort: &Sort,
    ) -> Result<(Vec<Transaction>, i64), AppError> {
        // Validate sort field to prevent SQL injection
        let sort_column = match sort.field.as_str() {
            "date" => "t.date",
            "amount" => "t.amount",
            "description" => "t.description",
            "category" => "t.category",
            "created_at" => "t.created_at",
            _ => "t.date",
        };
        let sort_dir = if sort.direction.to_lowercase() == "asc" {
            "ASC"
        } else {
            "DESC"
        };

        // Clamp per_page to MAX_PER_PAGE to prevent oversized queries.
        let per_page = pagination.per_page.clamp(1, Self::MAX_PER_PAGE);
        let offset = (pagination.page.max(1) - 1) * per_page;

        // Build the query dynamically but with parameterized values.
        // We use a common base WHERE clause that both count and data queries share.
        let rows = sqlx::query_as::<_, Transaction>(&format!(
            r#"
                SELECT t.id, t.account_id, t.date, t.amount, t.description,
                       t.original_description, t.category, t.subcategory,
                       t.merchant_name, t.tags, t.notes, t.is_recurring,
                       t.recurring_group_id, t.llm_confidence, t.user_overridden,
                       t.dedup_hash, t.created_at
                FROM transactions t
                LEFT JOIN accounts a ON a.id = t.account_id
                WHERE ($1::uuid IS NULL OR t.account_id = $1)
                  AND ($2::uuid IS NULL OR a.portfolio_id = $2)
                  AND ($3::date IS NULL OR t.date >= $3)
                  AND ($4::date IS NULL OR t.date <= $4)
                  AND ($5::text IS NULL OR t.category = $5)
                  AND ($6::numeric IS NULL OR t.amount >= $6)
                  AND ($7::numeric IS NULL OR t.amount <= $7)
                  AND ($8::text IS NULL OR t.description ILIKE '%' || $8 || '%')
                ORDER BY {} {}
                LIMIT $9 OFFSET $10
                "#,
            sort_column, sort_dir
        ))
        .bind(filters.account_id)
        .bind(filters.portfolio_id)
        .bind(filters.date_from)
        .bind(filters.date_to)
        .bind(&filters.category)
        .bind(filters.amount_min)
        .bind(filters.amount_max)
        .bind(&filters.search_text)
        .bind(per_page)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        let total = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
            FROM transactions t
            LEFT JOIN accounts a ON a.id = t.account_id
            WHERE ($1::uuid IS NULL OR t.account_id = $1)
              AND ($2::uuid IS NULL OR a.portfolio_id = $2)
              AND ($3::date IS NULL OR t.date >= $3)
              AND ($4::date IS NULL OR t.date <= $4)
              AND ($5::text IS NULL OR t.category = $5)
              AND ($6::numeric IS NULL OR t.amount >= $6)
              AND ($7::numeric IS NULL OR t.amount <= $7)
              AND ($8::text IS NULL OR t.description ILIKE '%' || $8 || '%')
            "#,
        )
        .bind(filters.account_id)
        .bind(filters.portfolio_id)
        .bind(filters.date_from)
        .bind(filters.date_to)
        .bind(&filters.category)
        .bind(filters.amount_min)
        .bind(filters.amount_max)
        .bind(&filters.search_text)
        .fetch_one(&self.pool)
        .await?;

        Ok((rows, total))
    }

    /// Find a single transaction by ID.
    pub async fn find_by_id(&self, id: Uuid) -> Result<Transaction, AppError> {
        let txn = sqlx::query_as::<_, Transaction>(
            r#"
            SELECT id, account_id, date, amount, description, original_description,
                   category, subcategory, merchant_name, tags, notes, is_recurring,
                   recurring_group_id, llm_confidence, user_overridden, dedup_hash, created_at
            FROM transactions
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await?;

        Ok(txn)
    }

    /// Update the category (and optionally subcategory, merchant_name) for a single transaction.
    pub async fn update_category(
        &self,
        id: Uuid,
        category: &str,
        subcategory: Option<&str>,
        merchant_name: Option<&str>,
        user_overridden: bool,
    ) -> Result<(), AppError> {
        sqlx::query(
            r#"
            UPDATE transactions
            SET category = $2,
                subcategory = $3,
                merchant_name = $4,
                user_overridden = $5
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(category)
        .bind(subcategory)
        .bind(merchant_name)
        .bind(user_overridden)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Bulk update the category for multiple transactions.
    pub async fn bulk_update_category(
        &self,
        ids: &[Uuid],
        category: &str,
        subcategory: Option<&str>,
    ) -> Result<usize, AppError> {
        if ids.is_empty() {
            return Ok(0);
        }

        let result = sqlx::query(
            r#"
            UPDATE transactions
            SET category = $2,
                subcategory = $3,
                user_overridden = true
            WHERE id = ANY($1)
            "#,
        )
        .bind(ids)
        .bind(category)
        .bind(subcategory)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() as usize)
    }

    /// Batch update LLM categorization results.
    pub async fn update_llm_results(
        &self,
        results: &[LlmCategorizationUpdate],
    ) -> Result<(), AppError> {
        if results.is_empty() {
            return Ok(());
        }

        for chunk in results.chunks(Self::DB_CHUNK_SIZE) {
            let ids: Vec<Uuid> = chunk.iter().map(|r| r.transaction_id).collect();
            let categories: Vec<String> = chunk.iter().map(|r| r.category.clone()).collect();
            let subcategories: Vec<String> = chunk.iter().map(|r| r.subcategory.clone()).collect();
            let merchant_names: Vec<String> =
                chunk.iter().map(|r| r.merchant_name.clone()).collect();
            let confidences: Vec<f64> = chunk.iter().map(|r| r.llm_confidence).collect();

            sqlx::query(
                r#"
                UPDATE transactions AS t
                SET category = u.category,
                    subcategory = u.subcategory,
                    merchant_name = u.merchant_name,
                    llm_confidence = u.confidence
                FROM UNNEST($1::uuid[], $2::text[], $3::text[], $4::text[], $5::float8[])
                    AS u(id, category, subcategory, merchant_name, confidence)
                WHERE t.id = u.id AND t.user_overridden = false
                "#,
            )
            .bind(&ids)
            .bind(&categories)
            .bind(&subcategories)
            .bind(&merchant_names)
            .bind(&confidences)
            .execute(&self.pool)
            .await?;
        }

        Ok(())
    }

    /// Set the `source_tier` column for a batch of transactions.
    ///
    /// Used by the categorization cascade to record which tier produced the
    /// category assignment (e.g. `"merchant_lookup"`, `"pattern_engine"`, `"llm"`).
    pub async fn set_source_tier(
        &self,
        transaction_ids: &[Uuid],
        tier: &str,
    ) -> Result<(), AppError> {
        if transaction_ids.is_empty() {
            return Ok(());
        }

        for chunk in transaction_ids.chunks(Self::DB_CHUNK_SIZE) {
            sqlx::query(
                r#"
                UPDATE transactions
                SET source_tier = $1
                WHERE id = ANY($2)
                "#,
            )
            .bind(tier)
            .bind(chunk)
            .execute(&self.pool)
            .await?;
        }

        Ok(())
    }

    /// Find all uncategorized transactions for an account.
    pub async fn find_uncategorized(&self, account_id: Uuid) -> Result<Vec<Transaction>, AppError> {
        let rows = sqlx::query_as::<_, Transaction>(
            r#"
            SELECT id, account_id, date, amount, description, original_description,
                   category, subcategory, merchant_name, tags, notes, is_recurring,
                   recurring_group_id, llm_confidence, user_overridden, dedup_hash, created_at
            FROM transactions
            WHERE account_id = $1
              AND (category IS NULL OR category = '')
              AND user_overridden = false
            ORDER BY date DESC
            "#,
        )
        .bind(account_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    /// Full-text search on description for a portfolio.
    pub async fn search(
        &self,
        portfolio_id: Uuid,
        query: &str,
        limit: i64,
    ) -> Result<Vec<Transaction>, AppError> {
        let limit = limit.clamp(1, Self::MAX_PER_PAGE);
        let rows = sqlx::query_as::<_, Transaction>(
            r#"
            SELECT t.id, t.account_id, t.date, t.amount, t.description,
                   t.original_description, t.category, t.subcategory,
                   t.merchant_name, t.tags, t.notes, t.is_recurring,
                   t.recurring_group_id, t.llm_confidence, t.user_overridden,
                   t.dedup_hash, t.created_at
            FROM transactions t
            JOIN accounts a ON a.id = t.account_id
            WHERE a.portfolio_id = $1
              AND t.description ILIKE '%' || $2 || '%'
            ORDER BY t.date DESC
            LIMIT $3
            "#,
        )
        .bind(portfolio_id)
        .bind(query)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    /// Fetch transactions for analysis across all accounts in a portfolio.
    ///
    /// Optionally filter by date range. Returns lightweight rows suitable
    /// for passing to `finima-analysis` functions.
    pub async fn list_for_analysis(
        &self,
        portfolio_id: Uuid,
        start_date: Option<NaiveDate>,
        end_date: Option<NaiveDate>,
    ) -> Result<Vec<TransactionForAnalysisRow>, AppError> {
        let rows = sqlx::query_as::<_, TransactionForAnalysisRow>(
            r#"
            SELECT t.id, t.date, t.amount, t.description,
                   t.merchant_name, t.category, t.subcategory, t.account_id,
                   t.direction
            FROM transactions t
            JOIN accounts a ON a.id = t.account_id
            WHERE a.portfolio_id = $1
              AND ($2::date IS NULL OR t.date >= $2)
              AND ($3::date IS NULL OR t.date <= $3)
            ORDER BY t.date
            "#,
        )
        .bind(portfolio_id)
        .bind(start_date)
        .bind(end_date)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    /// Fetch transactions for analysis for a single account.
    pub async fn list_by_account_for_analysis(
        &self,
        account_id: Uuid,
    ) -> Result<Vec<TransactionForAnalysisRow>, AppError> {
        let rows = sqlx::query_as::<_, TransactionForAnalysisRow>(
            r#"
            SELECT id, date, amount, description,
                   merchant_name, category, subcategory, account_id,
                   direction
            FROM transactions
            WHERE account_id = $1
            ORDER BY date
            "#,
        )
        .bind(account_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_pagination() {
        let p = Pagination::default();
        assert_eq!(p.page, 1);
        assert_eq!(p.per_page, 50);
    }

    #[test]
    fn default_sort() {
        let s = Sort::default();
        assert_eq!(s.field, "date");
        assert_eq!(s.direction, "desc");
    }

    #[test]
    fn max_per_page_constant() {
        assert_eq!(PgTransactionRepo::MAX_PER_PAGE, 100);
    }
}
