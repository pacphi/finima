use axum::extract::{Path, Query, State};
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use finima_auth::middleware::AuthUser;
use finima_core::traits::{AccountRepo, PortfolioRepo};
use finima_core::AppError;
use finima_db::{Pagination, Sort, TransactionFilters};

use crate::state::AppState;

// ---------------------------------------------------------------------------
// Request / response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct ListTransactionsQuery {
    pub account_id: Option<Uuid>,
    pub portfolio_id: Option<Uuid>,
    pub date_from: Option<chrono::NaiveDate>,
    pub date_to: Option<chrono::NaiveDate>,
    pub category: Option<String>,
    pub amount_min: Option<rust_decimal::Decimal>,
    pub amount_max: Option<rust_decimal::Decimal>,
    pub search_text: Option<String>,
    #[serde(default = "default_page")]
    pub page: i64,
    #[serde(default = "default_per_page")]
    pub per_page: i64,
    #[serde(default = "default_sort_field")]
    pub sort: String,
    #[serde(default = "default_sort_dir")]
    pub sort_dir: String,
}

fn default_page() -> i64 {
    1
}
fn default_per_page() -> i64 {
    50
}
fn default_sort_field() -> String {
    "date".to_string()
}
fn default_sort_dir() -> String {
    "desc".to_string()
}

#[derive(Debug, Serialize)]
pub struct PaginatedTransactions {
    pub data: Vec<finima_core::models::Transaction>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
}

#[derive(Debug, Deserialize)]
pub struct UpdateCategoryRequest {
    pub category: String,
    pub subcategory: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct BulkUpdateRequest {
    pub transaction_ids: Vec<Uuid>,
    pub category: String,
    pub subcategory: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct BulkUpdateResponse {
    pub updated: usize,
}

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: String,
    pub portfolio_id: Uuid,
    #[serde(default = "default_search_limit")]
    pub limit: i64,
}

fn default_search_limit() -> i64 {
    50
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// GET /api/transactions
///
/// Query params for filters, pagination, sort. Return paginated list with total count.
pub async fn list_transactions(
    user: AuthUser,
    State(state): State<AppState>,
    Query(params): Query<ListTransactionsQuery>,
) -> Result<impl IntoResponse, AppError> {
    // Verify ownership: if portfolio_id is given, check the user owns it.
    // If account_id is given, trace account -> portfolio -> user.
    // At least one scope must be provided.
    match (params.portfolio_id, params.account_id) {
        (Some(pid), _) => {
            state
                .portfolio_repo()
                .verify_ownership(pid, user.user_id)
                .await?;
        }
        (None, Some(aid)) => {
            let account = state.account_repo().find_by_id(aid).await?;
            state
                .portfolio_repo()
                .verify_ownership(account.portfolio_id, user.user_id)
                .await?;
        }
        (None, None) => {
            return Err(AppError::BadRequest(
                "Either portfolio_id or account_id is required".to_string(),
            ));
        }
    }

    let filters = TransactionFilters {
        account_id: params.account_id,
        portfolio_id: params.portfolio_id,
        date_from: params.date_from,
        date_to: params.date_to,
        category: params.category,
        amount_min: params.amount_min,
        amount_max: params.amount_max,
        search_text: params.search_text,
    };

    let pagination = Pagination {
        page: params.page,
        per_page: params.per_page,
    };

    let sort = Sort {
        field: params.sort,
        direction: params.sort_dir,
    };

    let (data, total) = state
        .transaction_repo()
        .list(&filters, &pagination, &sort)
        .await?;

    Ok(Json(PaginatedTransactions {
        data,
        total,
        page: pagination.page,
        per_page: pagination.per_page,
    }))
}

/// PUT /api/transactions/:id
///
/// Update a transaction's category. Sets user_overridden = true.
pub async fn update_transaction(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateCategoryRequest>,
) -> Result<impl IntoResponse, AppError> {
    if body.category.trim().is_empty() {
        return Err(AppError::BadRequest("Category is required".to_string()));
    }

    // Verify ownership: transaction -> account -> portfolio -> user
    let txn = state.transaction_repo().find_by_id(id).await?;
    let account = state.account_repo().find_by_id(txn.account_id).await?;
    state
        .portfolio_repo()
        .verify_ownership(account.portfolio_id, user.user_id)
        .await?;

    state
        .transaction_repo()
        .update_category(
            id,
            &body.category,
            body.subcategory.as_deref(),
            None,
            true, // user_overridden
        )
        .await?;

    let txn = state.transaction_repo().find_by_id(id).await?;
    Ok(Json(txn))
}

/// POST /api/transactions/bulk-update
///
/// Bulk update category for multiple transactions.
pub async fn bulk_update_transactions(
    user: AuthUser,
    State(state): State<AppState>,
    Json(body): Json<BulkUpdateRequest>,
) -> Result<impl IntoResponse, AppError> {
    if body.transaction_ids.is_empty() {
        return Err(AppError::BadRequest(
            "transaction_ids must not be empty".to_string(),
        ));
    }
    if body.category.trim().is_empty() {
        return Err(AppError::BadRequest("Category is required".to_string()));
    }

    // Verify ownership for every transaction in the batch.
    // Collect unique account IDs to minimize lookups.
    let mut verified_portfolios = std::collections::HashSet::new();
    for txn_id in &body.transaction_ids {
        let txn = state.transaction_repo().find_by_id(*txn_id).await?;
        if !verified_portfolios.contains(&txn.account_id) {
            let account = state.account_repo().find_by_id(txn.account_id).await?;
            state
                .portfolio_repo()
                .verify_ownership(account.portfolio_id, user.user_id)
                .await?;
            verified_portfolios.insert(txn.account_id);
        }
    }

    let updated = state
        .transaction_repo()
        .bulk_update_category(
            &body.transaction_ids,
            &body.category,
            body.subcategory.as_deref(),
        )
        .await?;

    Ok(Json(BulkUpdateResponse { updated }))
}

/// GET /api/transactions/search
///
/// Full-text search on transaction descriptions.
pub async fn search_transactions(
    user: AuthUser,
    State(state): State<AppState>,
    Query(params): Query<SearchQuery>,
) -> Result<impl IntoResponse, AppError> {
    // Verify the user owns the portfolio being searched
    state
        .portfolio_repo()
        .verify_ownership(params.portfolio_id, user.user_id)
        .await?;

    if params.q.trim().is_empty() {
        return Err(AppError::BadRequest(
            "Search query 'q' is required".to_string(),
        ));
    }

    let results = state
        .transaction_repo()
        .search(params.portfolio_id, &params.q, params.limit)
        .await?;

    Ok(Json(results))
}
