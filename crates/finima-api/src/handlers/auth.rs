use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};

use finima_auth::middleware::AuthUser;
use finima_auth::{jwt, magic_link};
use finima_core::AppError;

use crate::state::AppState;

// ---------------------------------------------------------------------------
// Request / response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct MagicLinkRequest {
    pub email: String,
}

#[derive(Debug, Deserialize)]
pub struct VerifyRequest {
    pub token: String,
    pub email: String,
}

#[derive(Debug, Deserialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

#[derive(Debug, Serialize)]
pub struct AuthTokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub user: UserResponse,
}

#[derive(Debug, Serialize)]
pub struct UserResponse {
    pub id: String,
    pub email: String,
    pub display_name: String,
}

#[derive(Debug, Serialize)]
pub struct MessageResponse {
    pub message: String,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// POST /api/auth/magic-link
///
/// Accept an email address, generate a magic link token, store its hash in
/// the database, and send the link via the configured email sender.
pub async fn request_magic_link(
    State(state): State<AppState>,
    Json(body): Json<MagicLinkRequest>,
) -> Result<impl IntoResponse, AppError> {
    let email = body.email.trim().to_lowercase();

    // Basic email validation
    if !email.contains('@') || !email.contains('.') || email.len() < 5 {
        return Err(AppError::BadRequest("Invalid email address".to_string()));
    }

    let (raw_token, token_hash) = magic_link::generate_token();

    let expiry_minutes = state.config().auth.magic_link_expiry_minutes as i64;
    let expires_at = chrono::Utc::now() + chrono::Duration::minutes(expiry_minutes);

    state
        .magic_link_repo()
        .create_magic_link(&email, &token_hash, expires_at)
        .await?;

    // Build the verification URL. In production this would be the frontend URL;
    // for now we use the server address.
    let base_url = format!(
        "http://{}:{}",
        state.config().server.host,
        state.config().server.port
    );
    let link_url = magic_link::build_magic_link_url(&base_url, &raw_token, &email);

    state
        .email_sender()
        .send_magic_link(&email, &link_url)
        .await
        .map_err(|e| AppError::InternalError(e.to_string()))?;

    Ok((
        StatusCode::OK,
        Json(MessageResponse {
            message: "Magic link sent. Check your email.".to_string(),
        }),
    ))
}

/// POST /api/auth/verify
///
/// Verify a magic link token: hash the raw token, look it up, check expiry
/// and used status, find or create the user, mark the link as used, and
/// return JWT access + refresh tokens.
pub async fn verify_magic_link(
    State(state): State<AppState>,
    Json(body): Json<VerifyRequest>,
) -> Result<impl IntoResponse, AppError> {
    let email = body.email.trim().to_lowercase();
    let token_hash = magic_link::hash_token(&body.token);

    let link = state
        .magic_link_repo()
        .find_by_token_hash(&token_hash)
        .await?
        .ok_or(AppError::Unauthorized)?;

    // Validate not expired
    if link.expires_at < chrono::Utc::now() {
        return Err(AppError::Unauthorized);
    }

    // Validate not already used
    if link.used_at.is_some() {
        return Err(AppError::Unauthorized);
    }

    // Validate email matches
    if link.email != email {
        return Err(AppError::Unauthorized);
    }

    // Find or create the user
    let user = match state.user_repo().find_by_email(&email).await? {
        Some(user) => user,
        None => {
            let display_name = email.split('@').next().unwrap_or("User").to_string();
            state.user_repo().create_user(&email, &display_name).await?
        }
    };

    // Mark the magic link as used
    state.magic_link_repo().mark_used(link.id).await?;

    // Issue tokens
    let jwt_secret = &state.config().auth.jwt_secret;
    let access_token = jwt::encode_access_token(user.id, &user.email, jwt_secret)
        .map_err(|e| AppError::InternalError(e.to_string()))?;
    let refresh_token = jwt::encode_refresh_token(user.id, jwt_secret)
        .map_err(|e| AppError::InternalError(e.to_string()))?;

    // Create a server-side session bound to the refresh token hash
    let refresh_hash = magic_link::hash_token(&refresh_token);
    let session_expires_at = chrono::Utc::now() + chrono::Duration::days(7);
    state
        .session_repo()
        .create_session(user.id, &refresh_hash, session_expires_at)
        .await?;

    Ok(Json(AuthTokenResponse {
        access_token,
        refresh_token,
        user: UserResponse {
            id: user.id.to_string(),
            email: user.email,
            display_name: user.display_name,
        },
    }))
}

/// POST /api/auth/refresh
///
/// Validate the refresh token against the server-side session table, delete
/// the old session, issue a new access + refresh token pair, and create a
/// new session (single-use rotation).
pub async fn refresh_token(
    State(state): State<AppState>,
    Json(body): Json<RefreshRequest>,
) -> Result<impl IntoResponse, AppError> {
    let jwt_secret = &state.config().auth.jwt_secret;

    // Decode and verify this is actually a refresh token
    let claims = jwt::decode_refresh_token(&body.refresh_token, jwt_secret)
        .map_err(|_| AppError::Unauthorized)?;

    // Look up the server-side session by the refresh token's hash
    let refresh_hash = magic_link::hash_token(&body.refresh_token);
    let session = state
        .session_repo()
        .find_by_refresh_hash(&refresh_hash)
        .await?
        .ok_or(AppError::Unauthorized)?;

    // Reject expired sessions
    if session.expires_at < chrono::Utc::now() {
        // Clean up the stale record
        let _ = state.session_repo().delete_session(session.id).await;
        return Err(AppError::Unauthorized);
    }

    // Invalidate the old session (single-use)
    state.session_repo().delete_session(session.id).await?;

    let user_id: uuid::Uuid = claims.sub.parse().map_err(|_| AppError::Unauthorized)?;

    // Look up the user to get current email (it may have changed)
    let user = state.user_repo().find_by_id(user_id).await?;

    // Issue new token pair
    let access_token = jwt::encode_access_token(user.id, &user.email, jwt_secret)
        .map_err(|e| AppError::InternalError(e.to_string()))?;
    let new_refresh_token = jwt::encode_refresh_token(user.id, jwt_secret)
        .map_err(|e| AppError::InternalError(e.to_string()))?;

    // Create a new session for the rotated refresh token
    let new_refresh_hash = magic_link::hash_token(&new_refresh_token);
    let session_expires_at = chrono::Utc::now() + chrono::Duration::days(7);
    state
        .session_repo()
        .create_session(user.id, &new_refresh_hash, session_expires_at)
        .await?;

    Ok(Json(AuthTokenResponse {
        access_token,
        refresh_token: new_refresh_token,
        user: UserResponse {
            id: user.id.to_string(),
            email: user.email,
            display_name: user.display_name,
        },
    }))
}

/// DELETE /api/auth/session
///
/// Log out the current user by deleting all of their server-side sessions.
/// Returns 204 No Content on success.
pub async fn delete_session(
    State(state): State<AppState>,
    user: AuthUser,
) -> Result<impl IntoResponse, AppError> {
    state
        .session_repo()
        .delete_all_user_sessions(user.user_id)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

// We need the UserRepo trait in scope for the find_by_email / create_user calls
use finima_core::traits::UserRepo;
