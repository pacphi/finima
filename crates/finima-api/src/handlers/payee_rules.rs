use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use finima_auth::middleware::AuthUser;
use finima_core::traits::PortfolioRepo;
use finima_core::AppError;

use crate::state::AppState;

// ---------------------------------------------------------------------------
// Request / response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct PayeeRulesQuery {
    pub portfolio_id: Uuid,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct PayeeSummary {
    pub merchant_name: String,
    pub category: Option<String>,
    pub subcategory: Option<String>,
    pub transaction_count: i64,
}

#[derive(Debug, Deserialize)]
pub struct ApplyPayeeRuleRequest {
    pub portfolio_id: Uuid,
    pub merchant_name: String,
    pub new_category: String,
    pub new_subcategory: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ApplyPayeeRuleResponse {
    pub updated: i64,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// GET /api/payee-rules?portfolio_id=
///
/// Returns distinct payees with their current category and transaction count
/// for all accounts in the given portfolio.
pub async fn list_payee_rules(
    user: AuthUser,
    State(state): State<AppState>,
    Query(params): Query<PayeeRulesQuery>,
) -> Result<impl IntoResponse, AppError> {
    state
        .portfolio_repo()
        .verify_ownership(params.portfolio_id, user.user_id)
        .await?;

    let rows = sqlx::query_as::<_, PayeeSummary>(
        r#"
        SELECT
            COALESCE(NULLIF(t.merchant_name, ''), t.description) AS merchant_name,
            t.category,
            t.subcategory,
            COUNT(*) AS transaction_count
        FROM transactions t
        JOIN accounts a ON a.id = t.account_id
        WHERE a.portfolio_id = $1
          AND a.is_archived = false
        GROUP BY COALESCE(NULLIF(t.merchant_name, ''), t.description), t.category, t.subcategory
        ORDER BY COUNT(*) DESC
        "#,
    )
    .bind(params.portfolio_id)
    .fetch_all(state.pool())
    .await
    .map_err(|e| AppError::InternalError(format!("Failed to query payee rules: {e}")))?;

    Ok(Json(rows))
}

/// POST /api/payee-rules/apply
///
/// Bulk-reassign a category for all transactions matching a merchant name
/// within a portfolio, and create/update a user override so future imports
/// are categorized automatically.
pub async fn apply_payee_rule(
    user: AuthUser,
    State(state): State<AppState>,
    Json(body): Json<ApplyPayeeRuleRequest>,
) -> Result<impl IntoResponse, AppError> {
    let merchant = body.merchant_name.trim();
    let category = body.new_category.trim();

    if merchant.is_empty() {
        return Err(AppError::BadRequest(
            "merchant_name is required".to_string(),
        ));
    }
    if category.is_empty() {
        return Err(AppError::BadRequest("new_category is required".to_string()));
    }

    state
        .portfolio_repo()
        .verify_ownership(body.portfolio_id, user.user_id)
        .await?;

    let subcategory = body
        .new_subcategory
        .as_deref()
        .unwrap_or("")
        .trim()
        .to_string();

    // 1. Bulk update all matching transactions in the portfolio.
    let result = sqlx::query(
        r#"
        UPDATE transactions
        SET category = $1,
            subcategory = $2,
            user_overridden = true
        WHERE COALESCE(NULLIF(merchant_name, ''), description) = $3
          AND account_id IN (
              SELECT id FROM accounts
              WHERE portfolio_id = $4 AND is_archived = false
          )
        "#,
    )
    .bind(category)
    .bind(&subcategory)
    .bind(merchant)
    .bind(body.portfolio_id)
    .execute(state.pool())
    .await
    .map_err(|e| AppError::InternalError(format!("Failed to update transactions: {e}")))?;

    let updated = result.rows_affected() as i64;

    // 2. Create or update a user override so future imports auto-categorize.
    state
        .override_repo()
        .create_or_update(user.user_id, merchant, category, &subcategory)
        .await?;

    Ok((StatusCode::OK, Json(ApplyPayeeRuleResponse { updated })))
}
