use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use finima_auth::middleware::AuthUser;
use finima_core::AppError;

use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct CategoryResponse {
    pub key: String,
    pub label: String,
    /// Whether this category is from the system config (not deletable) or user-created.
    pub is_system: bool,
}

#[derive(Debug, Deserialize)]
pub struct CreateCategoryRequest {
    pub key: String,
    pub label: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateCategoryRequest {
    pub label: String,
}

/// GET /api/categories
///
/// Returns the merged list of system + user-custom categories.
/// System categories come from config/default.yaml; user categories from the database.
pub async fn list_categories(user: AuthUser, State(state): State<AppState>) -> impl IntoResponse {
    let mut categories: Vec<CategoryResponse> = state
        .config()
        .categories
        .iter()
        .map(|c| CategoryResponse {
            key: c.key.clone(),
            label: c.label.clone(),
            is_system: true,
        })
        .collect();

    // Load user custom categories
    if let Ok(custom) = sqlx::query_as::<_, (String, String)>(
        "SELECT key, label FROM custom_categories WHERE user_id = $1 ORDER BY key",
    )
    .bind(user.user_id)
    .fetch_all(state.pool())
    .await
    {
        for (key, label) in custom {
            // Override system label if same key, or add new
            if let Some(existing) = categories.iter_mut().find(|c| c.key == key) {
                existing.label = label;
            } else {
                categories.push(CategoryResponse {
                    key,
                    label,
                    is_system: false,
                });
            }
        }
    }

    Json(categories)
}

/// POST /api/categories
///
/// Create a user-custom category. The key must be unique per user.
pub async fn create_category(
    user: AuthUser,
    State(state): State<AppState>,
    Json(body): Json<CreateCategoryRequest>,
) -> Result<impl IntoResponse, AppError> {
    let key = body.key.trim().to_lowercase().replace(' ', "_");
    let label = body.label.trim().to_string();

    if key.is_empty() || label.is_empty() {
        return Err(AppError::BadRequest(
            "Key and label are required".to_string(),
        ));
    }

    // Validate key format: only lowercase alphanumeric and underscores
    if !key
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    {
        return Err(AppError::BadRequest(
            "Key must contain only lowercase letters, numbers, and underscores".to_string(),
        ));
    }

    let id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO custom_categories (id, user_id, key, label)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (user_id, key) DO UPDATE SET label = EXCLUDED.label
        "#,
    )
    .bind(id)
    .bind(user.user_id)
    .bind(&key)
    .bind(&label)
    .execute(state.pool())
    .await
    .map_err(|e| AppError::InternalError(format!("Failed to create category: {}", e)))?;

    Ok((
        StatusCode::CREATED,
        Json(CategoryResponse {
            key,
            label,
            is_system: false,
        }),
    ))
}

/// PUT /api/categories/:key
///
/// Update the label for a category. Works for both system overrides and custom categories.
pub async fn update_category(
    user: AuthUser,
    State(state): State<AppState>,
    Path(key): Path<String>,
    Json(body): Json<UpdateCategoryRequest>,
) -> Result<impl IntoResponse, AppError> {
    let label = body.label.trim().to_string();
    if label.is_empty() {
        return Err(AppError::BadRequest("Label is required".to_string()));
    }

    let id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO custom_categories (id, user_id, key, label)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (user_id, key) DO UPDATE SET label = EXCLUDED.label
        "#,
    )
    .bind(id)
    .bind(user.user_id)
    .bind(&key)
    .bind(&label)
    .execute(state.pool())
    .await
    .map_err(|e| AppError::InternalError(format!("Failed to update category: {}", e)))?;

    Ok(Json(CategoryResponse {
        key,
        label,
        is_system: false,
    }))
}

/// DELETE /api/categories/:key
///
/// Delete a user-custom category. System categories cannot be deleted,
/// but user overrides of system categories can be removed (restores default label).
pub async fn delete_category(
    user: AuthUser,
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let result = sqlx::query("DELETE FROM custom_categories WHERE user_id = $1 AND key = $2")
        .bind(user.user_id)
        .bind(&key)
        .execute(state.pool())
        .await
        .map_err(|e| AppError::InternalError(format!("Failed to delete category: {}", e)))?;

    if result.rows_affected() == 0 {
        // Check if it's a system category
        let is_system = state.config().categories.iter().any(|c| c.key == key);
        if is_system {
            return Err(AppError::BadRequest(
                "System categories cannot be deleted".to_string(),
            ));
        }
        return Err(AppError::NotFound);
    }

    Ok(StatusCode::NO_CONTENT)
}
