use axum::extract::{Path, Query, State};
use axum::response::IntoResponse;
use axum::Json;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use finima_auth::middleware::AuthUser;
use finima_core::traits::PortfolioRepo;
use finima_core::{billing_cycle_month, start_of_month, AppError, Frequency};
use finima_db::RecurringGroupUpdate;

use crate::state::AppState;

use super::helpers::resolve_portfolio_id;

// ---------------------------------------------------------------------------
// Request types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct ListRecurringQuery {
    pub portfolio_id: Option<Uuid>,
    // `upcoming` and `days` are consumed downstream (or currently ignored
    // by list_recurring); keeping them here so Axum deserialization
    // accepts the full querystring the frontend sends.
    #[allow(dead_code)]
    pub upcoming: Option<bool>,
    #[allow(dead_code)]
    pub days: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct RecurringActionRequest {
    /// "confirm" or "dismiss" — or update fields directly
    pub action: Option<String>,
    #[serde(flatten)]
    pub update: Option<RecurringGroupUpdate>,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// GET /api/recurring?portfolio_id=...
///
/// List recurring groups. If `portfolio_id` is provided, it is validated
/// against the caller's ownership and used; otherwise the user's first
/// portfolio is returned.
pub async fn list_recurring(
    user: AuthUser,
    State(state): State<AppState>,
    Query(params): Query<ListRecurringQuery>,
) -> Result<impl IntoResponse, AppError> {
    let portfolio_id = resolve_portfolio_id(&user, &state, params.portfolio_id).await?;

    let groups = state
        .recurring_repo()
        .list_by_portfolio(portfolio_id)
        .await?;

    Ok(Json(groups))
}

/// Look up the portfolio_id that owns a recurring group.
async fn recurring_group_portfolio_id(state: &AppState, group_id: Uuid) -> Result<Uuid, AppError> {
    let row: (Uuid,) = sqlx::query_as("SELECT portfolio_id FROM recurring_groups WHERE id = $1")
        .bind(group_id)
        .fetch_one(state.pool())
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => AppError::NotFound,
            _ => AppError::from(e),
        })?;
    Ok(row.0)
}

/// A single observed posting for a recurring group, with both the raw
/// posting date (cash-flow reality) and the attributed billing cycle
/// month (cadence-anchored view).
///
/// Two month fields solve two different questions:
///   * `posted_month` — "what hit my account in October?" (reports, Sankey)
///   * `cycle_month`  — "which November bill was this?" (Recurring surface,
///     forecasting)
///
/// Matches Copilot's two-view semantics. See `docs/ADRs/ADR-020-*`.
#[derive(Debug, Serialize)]
pub struct RecurringOccurrence {
    pub transaction_id: Uuid,
    pub posting_date: NaiveDate,
    pub posted_month: NaiveDate,
    pub cycle_month: NaiveDate,
    pub amount: Decimal,
}

/// GET /api/recurring/:id/occurrences
///
/// Returns every transaction tied to the recurring group, each annotated
/// with its posting-date month (calendar bucketing) and its billing-cycle
/// month (cadence-anchored bucketing).
///
/// If the group has no `next_expected_date` (e.g. Variable cadence) or
/// the posting falls outside the cadence tolerance, `cycle_month` equals
/// `posted_month`.
pub async fn list_recurring_occurrences(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let portfolio_id = recurring_group_portfolio_id(&state, id).await?;
    state
        .portfolio_repo()
        .verify_ownership(portfolio_id, user.user_id)
        .await?;

    let group = sqlx::query_as::<_, finima_core::models::RecurringGroup>(
        r#"SELECT id, portfolio_id, merchant_name, category, frequency,
                  avg_amount, is_confirmed, next_expected_date, metadata
           FROM recurring_groups WHERE id = $1"#,
    )
    .bind(id)
    .fetch_one(state.pool())
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => AppError::NotFound,
        _ => AppError::from(e),
    })?;

    let txns = state.transaction_repo().list_by_recurring_group(id).await?;

    let cadence: Frequency = group.frequency;
    let anchor = group.next_expected_date;

    let occurrences: Vec<RecurringOccurrence> = txns
        .into_iter()
        .map(|t| {
            let posted_month = start_of_month(t.date);
            let cycle_month = match anchor {
                Some(a) => billing_cycle_month(t.date, cadence, a),
                None => posted_month,
            };
            RecurringOccurrence {
                transaction_id: t.id,
                posting_date: t.date,
                posted_month,
                cycle_month,
                amount: t.amount,
            }
        })
        .collect();

    Ok(Json(occurrences))
}

/// PUT /api/recurring/:id
///
/// Confirm, dismiss, or update a recurring group.
pub async fn update_recurring(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<RecurringActionRequest>,
) -> Result<impl IntoResponse, AppError> {
    // Verify ownership: recurring group -> portfolio -> user
    let portfolio_id = recurring_group_portfolio_id(&state, id).await?;
    state
        .portfolio_repo()
        .verify_ownership(portfolio_id, user.user_id)
        .await?;

    match body.action.as_deref() {
        Some("confirm") => {
            state.recurring_repo().confirm(id).await?;
            Ok(Json(serde_json::json!({"status": "confirmed"})))
        }
        Some("dismiss") => {
            state.recurring_repo().dismiss(id).await?;
            Ok(Json(serde_json::json!({"status": "dismissed"})))
        }
        _ => {
            // Treat as a field update
            if let Some(update) = body.update {
                let group = state.recurring_repo().update(id, &update).await?;
                Ok(Json(serde_json::to_value(group).unwrap_or_default()))
            } else {
                Err(AppError::BadRequest(
                    "Provide 'action' (confirm/dismiss) or update fields".to_string(),
                ))
            }
        }
    }
}
