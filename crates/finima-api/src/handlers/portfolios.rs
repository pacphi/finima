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

/// DELETE /api/portfolios/:id
///
/// **Destructive.** Permanently deletes the portfolio and every associated
/// row (accounts, budgets, savings_goals, recurring_groups, flow_groups,
/// account_flows, flow_patterns, embedding_index, and transactively
/// transactions/uploads via accounts through ON DELETE CASCADE), as well as
/// any object-storage artifacts for uploads owned by the portfolio's
/// accounts. Cannot be undone. The caller is expected to have prompted the
/// user for confirmation client-side.
pub async fn delete_portfolio(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    state
        .portfolio_repo()
        .verify_ownership(id, user.user_id)
        .await?;

    // Best-effort: remove any S3 objects for uploads belonging to accounts in
    // this portfolio before the DB cascade wipes the rows. Column mapping
    // JSON stores the `s3_key` set at preview time (see uploads::create_upload).
    // Failures here must not block deletion — we log and continue.
    let s3_keys: Vec<String> = sqlx::query_scalar::<_, Option<String>>(
        r#"
        SELECT u.column_mapping->>'s3_key'
        FROM uploads u
        JOIN accounts a ON a.id = u.account_id
        WHERE a.portfolio_id = $1
          AND u.column_mapping ? 's3_key'
        "#,
    )
    .bind(id)
    .fetch_all(state.pool())
    .await
    .unwrap_or_default()
    .into_iter()
    .flatten()
    .collect();

    for key in &s3_keys {
        if let Err(e) = state.object_storage().delete_object(key).await {
            tracing::warn!(
                portfolio_id = %id,
                s3_key = %key,
                error = %e,
                "Failed to delete S3 object during portfolio purge — continuing"
            );
        }
    }

    state.portfolio_repo().delete(id).await?;

    tracing::info!(
        portfolio_id = %id,
        s3_objects_deleted = s3_keys.len(),
        "Purged portfolio and all associated data"
    );

    Ok(StatusCode::NO_CONTENT)
}
