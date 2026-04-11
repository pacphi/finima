use axum::extract::{Path, State};
use axum::response::IntoResponse;
use axum::Json;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::Deserialize;
use uuid::Uuid;

use finima_auth::middleware::AuthUser;
use finima_core::traits::PortfolioRepo;
use finima_core::AppError;

use crate::state::AppState;

use super::helpers::first_portfolio_id;

// ---------------------------------------------------------------------------
// Request types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct CreateSavingsGoalRequest {
    pub name: String,
    pub target_amount: Decimal,
    pub target_date: Option<NaiveDate>,
    pub linked_account_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateSavingsGoalRequest {
    pub name: Option<String>,
    pub target_amount: Option<Decimal>,
    pub current_amount: Option<Decimal>,
    pub target_date: Option<NaiveDate>,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// GET /api/savings-goals
pub async fn list_goals(
    user: AuthUser,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let portfolio_id = first_portfolio_id(&user, &state).await?;

    let goals = state
        .savings_goal_repo()
        .list_by_portfolio(portfolio_id)
        .await?;

    Ok(Json(goals))
}

/// POST /api/savings-goals
pub async fn create_goal(
    user: AuthUser,
    State(state): State<AppState>,
    Json(body): Json<CreateSavingsGoalRequest>,
) -> Result<impl IntoResponse, AppError> {
    let portfolio_id = first_portfolio_id(&user, &state).await?;

    let goal = state
        .savings_goal_repo()
        .create(
            portfolio_id,
            &body.name,
            body.target_amount,
            body.target_date,
            body.linked_account_id,
        )
        .await?;

    Ok(Json(goal))
}

/// Look up the portfolio_id that owns a savings goal.
async fn savings_goal_portfolio_id(state: &AppState, goal_id: Uuid) -> Result<Uuid, AppError> {
    let row: (Uuid,) = sqlx::query_as("SELECT portfolio_id FROM savings_goals WHERE id = $1")
        .bind(goal_id)
        .fetch_one(state.pool())
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => AppError::NotFound,
            _ => AppError::from(e),
        })?;
    Ok(row.0)
}

/// PUT /api/savings-goals/:id
pub async fn update_goal(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateSavingsGoalRequest>,
) -> Result<impl IntoResponse, AppError> {
    // Verify ownership: savings goal -> portfolio -> user
    let portfolio_id = savings_goal_portfolio_id(&state, id).await?;
    state
        .portfolio_repo()
        .verify_ownership(portfolio_id, user.user_id)
        .await?;

    let goal = state
        .savings_goal_repo()
        .update(
            id,
            body.name.as_deref(),
            body.target_amount,
            body.current_amount,
            body.target_date,
        )
        .await?;

    Ok(Json(goal))
}

/// DELETE /api/savings-goals/:id
pub async fn delete_goal(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    // Verify ownership: savings goal -> portfolio -> user
    let portfolio_id = savings_goal_portfolio_id(&state, id).await?;
    state
        .portfolio_repo()
        .verify_ownership(portfolio_id, user.user_id)
        .await?;

    state.savings_goal_repo().delete(id).await?;

    Ok(Json(serde_json::json!({"status": "deleted"})))
}
