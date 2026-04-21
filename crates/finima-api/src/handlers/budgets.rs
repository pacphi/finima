use axum::extract::{Query, State};
use axum::response::IntoResponse;
use axum::Json;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use finima_analysis::{auto_suggest_budgets, compute_budget_vs_actual, BudgetEntry};
use finima_auth::middleware::AuthUser;
use finima_core::AppError;

use crate::state::AppState;

use super::helpers::{parse_month, resolve_portfolio_id, to_analysis};

// ---------------------------------------------------------------------------
// Request / response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct MonthQuery {
    pub month: Option<String>,
    pub portfolio_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct PortfolioQuery {
    pub portfolio_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct CreateBudgetRequest {
    pub category: String,
    pub monthly_limit: Decimal,
    #[serde(default)]
    pub rollover: bool,
    /// Month in YYYY-MM format.
    pub month: String,
    pub portfolio_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
pub struct BudgetVsActualEntry {
    pub category: String,
    pub limit: Decimal,
    pub spent: Decimal,
    pub remaining: Decimal,
    pub percentage: f64,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// GET /api/budgets?month=2026-04
pub async fn list_budgets(
    user: AuthUser,
    State(state): State<AppState>,
    Query(params): Query<MonthQuery>,
) -> Result<impl IntoResponse, AppError> {
    let portfolio_id = resolve_portfolio_id(&user, &state, params.portfolio_id).await?;
    let month = parse_month(&params.month)?;

    let budgets = state
        .budget_repo()
        .list_by_portfolio_month(portfolio_id, month)
        .await?;

    Ok(Json(budgets))
}

/// POST /api/budgets
pub async fn create_budget(
    user: AuthUser,
    State(state): State<AppState>,
    Json(body): Json<CreateBudgetRequest>,
) -> Result<impl IntoResponse, AppError> {
    let portfolio_id = resolve_portfolio_id(&user, &state, body.portfolio_id).await?;

    // Parse month string to NaiveDate (first of month).
    let month = parse_month(&Some(body.month.clone()))?;

    let budget = state
        .budget_repo()
        .create_or_update(
            portfolio_id,
            &body.category,
            body.monthly_limit,
            body.rollover,
            month,
        )
        .await?;

    Ok(Json(budget))
}

/// GET /api/budgets/vs-actual?month=2026-04
pub async fn budget_vs_actual(
    user: AuthUser,
    State(state): State<AppState>,
    Query(params): Query<MonthQuery>,
) -> Result<impl IntoResponse, AppError> {
    let portfolio_id = resolve_portfolio_id(&user, &state, params.portfolio_id).await?;
    let month = parse_month(&params.month)?;

    // Get budgets for the month.
    let budgets = state
        .budget_repo()
        .list_by_portfolio_month(portfolio_id, month)
        .await?;

    let budget_entries: Vec<BudgetEntry> = budgets
        .iter()
        .map(|b| BudgetEntry {
            category: b.category.clone(),
            limit: b.monthly_limit,
        })
        .collect();

    // Get transactions for the month.
    let txn_rows = state
        .transaction_repo()
        .list_for_analysis(portfolio_id, None, None)
        .await?;
    let txns = to_analysis(&txn_rows);

    let results = compute_budget_vs_actual(&budget_entries, &txns, month);

    let entries: Vec<BudgetVsActualEntry> = results
        .into_iter()
        .map(|r| BudgetVsActualEntry {
            category: r.category,
            limit: r.limit,
            spent: r.spent,
            remaining: r.remaining,
            percentage: r.percentage,
        })
        .collect();

    Ok(Json(entries))
}

/// POST /api/budgets/auto-suggest
pub async fn auto_suggest(
    user: AuthUser,
    State(state): State<AppState>,
    Query(params): Query<PortfolioQuery>,
) -> Result<impl IntoResponse, AppError> {
    let portfolio_id = resolve_portfolio_id(&user, &state, params.portfolio_id).await?;

    // Get last 3 months of transactions.
    let txn_rows = state
        .transaction_repo()
        .list_for_analysis(portfolio_id, None, None)
        .await?;
    let txns = to_analysis(&txn_rows);

    let suggestions = auto_suggest_budgets(&txns, 3);

    Ok(Json(suggestions))
}
