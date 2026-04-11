use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};

use finima_auth::middleware::AuthUser;
use finima_core::traits::UserRepo;
use finima_core::AppError;

use crate::state::AppState;

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct UserProfileResponse {
    pub id: String,
    pub email: String,
    pub display_name: String,
    pub preferences: serde_json::Value,
    pub created_at: String,
    pub updated_at: String,
}

// ---------------------------------------------------------------------------
// Request types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct UpdatePreferencesRequest {
    /// Partial JSON object to merge with existing preferences.
    #[serde(flatten)]
    pub preferences: serde_json::Value,
}

// ---------------------------------------------------------------------------
// GET /api/users/me
// ---------------------------------------------------------------------------

pub async fn get_current_user(
    auth: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<UserProfileResponse>, AppError> {
    let user = state.user_repo().find_by_id(auth.user_id).await?;

    Ok(Json(UserProfileResponse {
        id: user.id.to_string(),
        email: user.email,
        display_name: user.display_name,
        preferences: user.preferences,
        created_at: user.created_at.to_rfc3339(),
        updated_at: user.updated_at.to_rfc3339(),
    }))
}

// ---------------------------------------------------------------------------
// PUT /api/users/me/preferences
// ---------------------------------------------------------------------------

pub async fn update_preferences(
    auth: AuthUser,
    State(state): State<AppState>,
    Json(body): Json<UpdatePreferencesRequest>,
) -> Result<Json<UserProfileResponse>, AppError> {
    // Validate that the input is a JSON object.
    if !body.preferences.is_object() {
        return Err(AppError::BadRequest(
            "Preferences must be a JSON object".to_string(),
        ));
    }

    // Load existing preferences and merge.
    let existing_user = state.user_repo().find_by_id(auth.user_id).await?;
    let mut merged = existing_user.preferences.clone();

    if let (Some(existing_obj), Some(new_obj)) =
        (merged.as_object_mut(), body.preferences.as_object())
    {
        for (key, value) in new_obj {
            existing_obj.insert(key.clone(), value.clone());
        }
    }

    let user = state
        .user_repo()
        .update_preferences(auth.user_id, merged)
        .await?;

    Ok(Json(UserProfileResponse {
        id: user.id.to_string(),
        email: user.email,
        display_name: user.display_name,
        preferences: user.preferences,
        created_at: user.created_at.to_rfc3339(),
        updated_at: user.updated_at.to_rfc3339(),
    }))
}
