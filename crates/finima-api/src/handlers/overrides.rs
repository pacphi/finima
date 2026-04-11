use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::Deserialize;

use finima_auth::middleware::AuthUser;
use finima_core::AppError;

use crate::state::AppState;

// ---------------------------------------------------------------------------
// Request types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct CreateOverrideRequest {
    pub description_pattern: String,
    pub category: String,
    pub subcategory: Option<String>,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// POST /api/user-overrides
///
/// Create or update a user category override.
pub async fn create_override(
    user: AuthUser,
    State(state): State<AppState>,
    Json(body): Json<CreateOverrideRequest>,
) -> Result<impl IntoResponse, AppError> {
    if body.description_pattern.trim().is_empty() {
        return Err(AppError::BadRequest(
            "description_pattern is required".to_string(),
        ));
    }
    if body.category.trim().is_empty() {
        return Err(AppError::BadRequest("category is required".to_string()));
    }

    let subcategory = body.subcategory.as_deref().unwrap_or("");

    let result = state
        .override_repo()
        .create_or_update(
            user.user_id,
            &body.description_pattern,
            &body.category,
            subcategory,
        )
        .await?;

    Ok((StatusCode::CREATED, Json(result)))
}

/// GET /api/user-overrides
///
/// List all overrides for the authenticated user.
pub async fn list_overrides(
    user: AuthUser,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let overrides = state.override_repo().list_by_user(user.user_id).await?;
    Ok(Json(overrides))
}
