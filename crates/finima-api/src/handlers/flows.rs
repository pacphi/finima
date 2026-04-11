use std::collections::HashMap;

use axum::extract::{Path, Query, State};
use axum::response::IntoResponse;
use axum::Json;
use chrono::Datelike;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use finima_analysis::{build_outflow_ranking, build_sankey_data, build_waterfall, FlowRecord};
use finima_auth::middleware::AuthUser;
use finima_core::traits::{AccountRepo, PortfolioRepo};
use finima_core::AppError;
use finima_db::NewAccountFlow;

use crate::state::AppState;

use super::helpers::{first_portfolio_id, parse_month, to_analysis};

// ---------------------------------------------------------------------------
// Request / response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct MonthQuery {
    pub month: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct BalanceImpactQuery {
    pub month: Option<String>,
    pub account_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct CreateFlowRequest {
    pub source_transaction_id: Uuid,
    pub target_transaction_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct FlowActionRequest {
    /// "confirm" or "dismiss"
    pub action: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateFlowGroupRequest {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateFlowGroupRequest {
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct SankeyResponse {
    pub nodes: Vec<SankeyNodeResponse>,
    pub links: Vec<SankeyLinkResponse>,
}

#[derive(Debug, Serialize)]
pub struct SankeyNodeResponse {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Serialize)]
pub struct SankeyLinkResponse {
    pub source: String,
    pub target: String,
    pub value: Decimal,
}

// ---------------------------------------------------------------------------
// Flow handlers
// ---------------------------------------------------------------------------

/// GET /api/flows?month=2026-04
pub async fn list_flows(
    user: AuthUser,
    State(state): State<AppState>,
    Query(params): Query<MonthQuery>,
) -> Result<impl IntoResponse, AppError> {
    let portfolio_id = first_portfolio_id(&user, &state).await?;
    let month = parse_month(&params.month)?;

    let flows = state
        .flow_repo()
        .list_by_portfolio_month(portfolio_id, month)
        .await?;

    Ok(Json(flows))
}

/// POST /api/flows — manually link two transactions as a transfer pair.
pub async fn create_flow(
    user: AuthUser,
    State(state): State<AppState>,
    Json(body): Json<CreateFlowRequest>,
) -> Result<impl IntoResponse, AppError> {
    let portfolio_id = first_portfolio_id(&user, &state).await?;

    // Fetch both transactions to determine accounts and amount.
    let source_txn = state
        .transaction_repo()
        .find_by_id(body.source_transaction_id)
        .await?;
    let target_txn = state
        .transaction_repo()
        .find_by_id(body.target_transaction_id)
        .await?;

    let new_flow = NewAccountFlow {
        portfolio_id,
        source_account_id: source_txn.account_id,
        target_account_id: target_txn.account_id,
        source_transaction_id: Some(source_txn.id),
        target_transaction_id: Some(target_txn.id),
        amount: source_txn.amount.abs(),
        flow_date: source_txn.date,
        is_auto_detected: false,
    };

    let flow = state.flow_repo().create(&new_flow).await?;

    Ok(Json(flow))
}

/// PUT /api/flows/:id — confirm or dismiss a flow.
pub async fn update_flow(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<FlowActionRequest>,
) -> Result<impl IntoResponse, AppError> {
    // Verify ownership: flow -> portfolio -> user
    let flow = state.flow_repo().find_by_id(id).await?;
    state
        .portfolio_repo()
        .verify_ownership(flow.portfolio_id, user.user_id)
        .await?;

    match body.action.as_str() {
        "confirm" => {
            state.flow_repo().confirm(id).await?;
            Ok(Json(serde_json::json!({"status": "confirmed"})))
        }
        "dismiss" => {
            state.flow_repo().dismiss(id).await?;
            Ok(Json(serde_json::json!({"status": "dismissed"})))
        }
        _ => Err(AppError::BadRequest(
            "action must be 'confirm' or 'dismiss'".to_string(),
        )),
    }
}

/// DELETE /api/flows/:id
pub async fn delete_flow(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    // Verify ownership: flow -> portfolio -> user
    let flow = state.flow_repo().find_by_id(id).await?;
    state
        .portfolio_repo()
        .verify_ownership(flow.portfolio_id, user.user_id)
        .await?;

    state.flow_repo().delete(id).await?;

    Ok(Json(serde_json::json!({"status": "deleted"})))
}

/// GET /api/flows/sankey?month=2026-04
pub async fn get_sankey(
    user: AuthUser,
    State(state): State<AppState>,
    Query(params): Query<MonthQuery>,
) -> Result<impl IntoResponse, AppError> {
    let portfolio_id = first_portfolio_id(&user, &state).await?;
    let month = parse_month(&params.month)?;

    let db_flows = state
        .flow_repo()
        .list_by_portfolio_month(portfolio_id, month)
        .await?;

    // Build account name lookup.
    let accounts = state.account_repo().list_by_portfolio(portfolio_id).await?;
    let acct_names: HashMap<Uuid, String> =
        accounts.iter().map(|a| (a.id, a.name.clone())).collect();

    // Convert DB flows to analysis FlowRecords.
    let flow_records: Vec<FlowRecord> = db_flows
        .iter()
        .map(|f| FlowRecord {
            source_account_id: f.source_account_id,
            source_account_name: acct_names
                .get(&f.source_account_id)
                .cloned()
                .unwrap_or_else(|| "Unknown".to_string()),
            target_account_id: Some(f.target_account_id),
            target_account_name: acct_names.get(&f.target_account_id).cloned(),
            amount: f.amount,
            flow_date: f.flow_date,
            category: None,
        })
        .collect();

    let sankey = build_sankey_data(&flow_records, month);

    let response = SankeyResponse {
        nodes: sankey
            .nodes
            .into_iter()
            .map(|n| SankeyNodeResponse {
                id: n.id,
                label: n.label,
            })
            .collect(),
        links: sankey
            .links
            .into_iter()
            .map(|l| SankeyLinkResponse {
                source: l.source,
                target: l.target,
                value: l.value,
            })
            .collect(),
    };

    Ok(Json(response))
}

/// GET /api/flows/outflow-ranking?month=2026-04
pub async fn get_outflow_ranking(
    user: AuthUser,
    State(state): State<AppState>,
    Query(params): Query<MonthQuery>,
) -> Result<impl IntoResponse, AppError> {
    let portfolio_id = first_portfolio_id(&user, &state).await?;
    let month = parse_month(&params.month)?;

    let db_flows = state
        .flow_repo()
        .list_by_portfolio_month(portfolio_id, month)
        .await?;

    let accounts = state.account_repo().list_by_portfolio(portfolio_id).await?;
    let acct_names: HashMap<Uuid, String> =
        accounts.iter().map(|a| (a.id, a.name.clone())).collect();

    let flow_records: Vec<FlowRecord> = db_flows
        .iter()
        .map(|f| FlowRecord {
            source_account_id: f.source_account_id,
            source_account_name: acct_names
                .get(&f.source_account_id)
                .cloned()
                .unwrap_or_else(|| "Unknown".to_string()),
            target_account_id: Some(f.target_account_id),
            target_account_name: acct_names.get(&f.target_account_id).cloned(),
            amount: f.amount,
            flow_date: f.flow_date,
            category: acct_names.get(&f.target_account_id).cloned(),
        })
        .collect();

    // Compute total income for the month from transactions.
    let txn_rows = state
        .transaction_repo()
        .list_for_analysis(portfolio_id, Some(month), None)
        .await?;
    let total_income: Decimal = txn_rows
        .iter()
        .filter(|t| t.amount > Decimal::ZERO)
        .map(|t| t.amount)
        .sum();

    let ranking = build_outflow_ranking(&flow_records, total_income);

    Ok(Json(ranking))
}

/// GET /api/flows/balance-impact?month=2026-04&account_id=uuid
pub async fn get_balance_impact(
    user: AuthUser,
    State(state): State<AppState>,
    Query(params): Query<BalanceImpactQuery>,
) -> Result<impl IntoResponse, AppError> {
    let portfolio_id = first_portfolio_id(&user, &state).await?;
    let month = parse_month(&params.month)?;

    let account = state.account_repo().find_by_id(params.account_id).await?;

    let db_flows = state
        .flow_repo()
        .list_by_portfolio_month(portfolio_id, month)
        .await?;

    let accounts = state.account_repo().list_by_portfolio(portfolio_id).await?;
    let acct_names: HashMap<Uuid, String> =
        accounts.iter().map(|a| (a.id, a.name.clone())).collect();

    // Get transactions for the account to compute income for this month.
    let txn_rows = state
        .transaction_repo()
        .list_by_account_for_analysis(params.account_id)
        .await?;
    let txns = to_analysis(&txn_rows);

    // Sum income for the month.
    let month_income: Decimal = txns
        .iter()
        .filter(|t| {
            t.date.year() == month.year()
                && t.date.month() == month.month()
                && t.amount > Decimal::ZERO
        })
        .map(|t| t.amount)
        .sum();

    // Build outflows from flows where this account is the source.
    let outflows: Vec<(String, Decimal)> = db_flows
        .iter()
        .filter(|f| f.source_account_id == params.account_id)
        .map(|f| {
            let label = acct_names
                .get(&f.target_account_id)
                .cloned()
                .unwrap_or_else(|| "Unknown".to_string());
            (label, f.amount)
        })
        .collect();

    let start_balance = account.opening_balance;
    let waterfall = build_waterfall(params.account_id, start_balance, month_income, &outflows);

    Ok(Json(waterfall))
}

// ---------------------------------------------------------------------------
// Flow group handlers
// ---------------------------------------------------------------------------

/// Look up the portfolio_id that owns a flow group.
async fn flow_group_portfolio_id(state: &AppState, group_id: Uuid) -> Result<Uuid, AppError> {
    let row: (Uuid,) = sqlx::query_as("SELECT portfolio_id FROM flow_groups WHERE id = $1")
        .bind(group_id)
        .fetch_one(state.pool())
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => AppError::NotFound,
            _ => AppError::from(e),
        })?;
    Ok(row.0)
}

/// GET /api/flow-groups
pub async fn list_flow_groups(
    user: AuthUser,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let portfolio_id = first_portfolio_id(&user, &state).await?;

    let groups = state
        .flow_group_repo()
        .list_by_portfolio(portfolio_id)
        .await?;

    Ok(Json(groups))
}

/// POST /api/flow-groups
pub async fn create_flow_group(
    user: AuthUser,
    State(state): State<AppState>,
    Json(body): Json<CreateFlowGroupRequest>,
) -> Result<impl IntoResponse, AppError> {
    let portfolio_id = first_portfolio_id(&user, &state).await?;

    let group = state
        .flow_group_repo()
        .create(portfolio_id, &body.name)
        .await?;

    Ok(Json(group))
}

/// PUT /api/flow-groups/:id
pub async fn update_flow_group(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateFlowGroupRequest>,
) -> Result<impl IntoResponse, AppError> {
    // Verify ownership: flow group -> portfolio -> user
    let portfolio_id = flow_group_portfolio_id(&state, id).await?;
    state
        .portfolio_repo()
        .verify_ownership(portfolio_id, user.user_id)
        .await?;

    let group = state.flow_group_repo().update(id, &body.name).await?;

    Ok(Json(group))
}

/// DELETE /api/flow-groups/:id
pub async fn delete_flow_group(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    // Verify ownership: flow group -> portfolio -> user
    let portfolio_id = flow_group_portfolio_id(&state, id).await?;
    state
        .portfolio_repo()
        .verify_ownership(portfolio_id, user.user_id)
        .await?;

    state.flow_group_repo().delete(id).await?;

    Ok(Json(serde_json::json!({"status": "deleted"})))
}
