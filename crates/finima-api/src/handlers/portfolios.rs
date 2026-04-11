use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::Deserialize;
use uuid::Uuid;

use finima_auth::middleware::AuthUser;
use finima_core::traits::PortfolioRepo;
use finima_core::AppError;

use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct CreatePortfolioRequest {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePortfolioRequest {
    pub name: String,
}

/// GET /api/portfolios
///
/// List all portfolios belonging to the authenticated user.
pub async fn list_portfolios(
    user: AuthUser,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let portfolios = state.portfolio_repo().list_by_user(user.user_id).await?;
    Ok(Json(portfolios))
}

/// POST /api/portfolios
///
/// Create a new portfolio for the authenticated user.
pub async fn create_portfolio(
    user: AuthUser,
    State(state): State<AppState>,
    Json(body): Json<CreatePortfolioRequest>,
) -> Result<impl IntoResponse, AppError> {
    let name = body.name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::BadRequest(
            "Portfolio name is required".to_string(),
        ));
    }

    let portfolio = state.portfolio_repo().create(user.user_id, &name).await?;
    Ok((StatusCode::CREATED, Json(portfolio)))
}

/// GET /api/portfolios/:id
///
/// Get a single portfolio by ID. Verifies the authenticated user owns it.
pub async fn get_portfolio(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    state
        .portfolio_repo()
        .verify_ownership(id, user.user_id)
        .await?;

    let portfolio = state.portfolio_repo().find_by_id(id).await?;
    Ok(Json(portfolio))
}

/// PUT /api/portfolios/:id
///
/// Update a portfolio's name. Verifies the authenticated user owns it.
pub async fn update_portfolio(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdatePortfolioRequest>,
) -> Result<impl IntoResponse, AppError> {
    let name = body.name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::BadRequest(
            "Portfolio name is required".to_string(),
        ));
    }

    state
        .portfolio_repo()
        .verify_ownership(id, user.user_id)
        .await?;

    let portfolio = state.portfolio_repo().update(id, &name).await?;
    Ok(Json(portfolio))
}
