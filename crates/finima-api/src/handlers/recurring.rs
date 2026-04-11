use axum::extract::{Path, State};
use axum::response::IntoResponse;
use axum::Json;
use serde::Deserialize;
use uuid::Uuid;

use finima_auth::middleware::AuthUser;
use finima_core::traits::PortfolioRepo;
use finima_core::AppError;
use finima_db::RecurringGroupUpdate;

use crate::state::AppState;

// ---------------------------------------------------------------------------
// Request types
// ---------------------------------------------------------------------------

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

/// GET /api/recurring
///
/// List recurring groups for the user's first portfolio.
/// In the future this should accept a portfolio_id query param.
pub async fn list_recurring(
    user: AuthUser,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    // Get the user's portfolios and use the first one
    let portfolios = state.portfolio_repo().list_by_user(user.user_id).await?;

    let portfolio = portfolios.first().ok_or(AppError::NotFound)?;

    let groups = state
        .recurring_repo()
        .list_by_portfolio(portfolio.id)
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
