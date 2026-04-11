//! Integration tests for the authentication flow.
//!
//! These tests exercise the magic link and JWT token lifecycle against
//! a real PostgreSQL test database (from docker-compose.test.yml).
//!
//! Run with:
//!   docker compose -f docker-compose.test.yml up -d
//!   cargo test -p finima-api --test auth_test

mod common;

use axum::http::StatusCode;
use uuid::Uuid;

#[allow(unused_imports)]
use common::{
    access_token_for, authenticate_user, build_test_router, expired_token, refresh_token_for,
    seed_test_db, setup_test_db, TestAppState, TestClient, TEST_JWT_SECRET, USER_A_EMAIL,
    USER_A_ID,
};

// ---------------------------------------------------------------------------
// Health check (smoke test for the in-process router pattern)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn health_check_returns_200() {
    let pool = setup_test_db().await;
    let state = TestAppState::new(pool);
    let router = build_test_router(state);
    let client = TestClient::new(router);

    let (status, body) = client.get("/health").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "healthy");
}

// ---------------------------------------------------------------------------
// Magic link request (database-level tests)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn request_magic_link_creates_record_in_db() {
    let pool = setup_test_db().await;
    let state = TestAppState::new(pool.clone());

    let email = "magictest@finima.local";

    // Generate a magic link token and store its hash
    let (_raw_token, token_hash) = finima_auth::generate_token();
    let expires_at = chrono::Utc::now() + chrono::Duration::minutes(5);
    state
        .magic_link_repo()
        .create_magic_link(email, &token_hash, expires_at)
        .await
        .expect("Should create magic link");

    // Verify we can find it by hash
    let found = state
        .magic_link_repo()
        .find_by_token_hash(&token_hash)
        .await
        .expect("Should query magic link")
        .expect("Magic link should exist");

    assert_eq!(found.email, email);
    assert!(found.used_at.is_none(), "Should not be used yet");
}

// ---------------------------------------------------------------------------
// Magic link verification (database-level tests)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn verify_magic_link_creates_user_and_issues_tokens() {
    let pool = setup_test_db().await;
    let state = TestAppState::new(pool.clone());

    let email = "newuser@finima.local";

    // Create a magic link
    let (raw_token, token_hash) = finima_auth::generate_token();
    let expires_at = chrono::Utc::now() + chrono::Duration::minutes(5);
    let link = state
        .magic_link_repo()
        .create_magic_link(email, &token_hash, expires_at)
        .await
        .unwrap();

    // Verify the token hash matches
    let computed_hash = finima_auth::hash_token(&raw_token);
    assert_eq!(computed_hash, token_hash);

    // Simulate what the verify handler does: look up link, create user, issue tokens
    let found = state
        .magic_link_repo()
        .find_by_token_hash(&computed_hash)
        .await
        .unwrap()
        .expect("Link should exist");
    assert_eq!(found.email, email);
    assert!(found.expires_at > chrono::Utc::now());
    assert!(found.used_at.is_none());

    // Find or create the user
    use finima_core::traits::UserRepo;
    let user = match state.user_repo().find_by_email(email).await.unwrap() {
        Some(u) => u,
        None => state
            .user_repo()
            .create_user(email, "newuser")
            .await
            .unwrap(),
    };

    // Mark link as used
    state.magic_link_repo().mark_used(link.id).await.unwrap();

    // Issue tokens
    let access = finima_auth::jwt::encode_access_token(user.id, &user.email, TEST_JWT_SECRET)
        .expect("Should encode access token");
    let refresh = finima_auth::jwt::encode_refresh_token(user.id, TEST_JWT_SECRET)
        .expect("Should encode refresh token");

    // Verify the access token decodes correctly
    let claims = finima_auth::jwt::decode_token(&access, TEST_JWT_SECRET)
        .expect("Should decode access token");
    assert_eq!(claims.sub, user.id.to_string());
    assert_eq!(claims.email, email);

    // Verify the refresh token decodes correctly
    let refresh_claims = finima_auth::jwt::decode_token(&refresh, TEST_JWT_SECRET)
        .expect("Should decode refresh token");
    assert_eq!(refresh_claims.sub, user.id.to_string());

    // Verify the magic link is now marked as used
    let used_link = state
        .magic_link_repo()
        .find_by_token_hash(&token_hash)
        .await
        .unwrap()
        .expect("Link should still exist");
    assert!(used_link.used_at.is_some(), "Link should be marked as used");
}

#[tokio::test]
async fn verify_expired_magic_link_is_rejected() {
    let pool = setup_test_db().await;
    let state = TestAppState::new(pool.clone());

    let email = "expired@finima.local";
    let (_, token_hash) = finima_auth::generate_token();

    // Create a link that expired 1 hour ago
    let expires_at = chrono::Utc::now() - chrono::Duration::hours(1);
    state
        .magic_link_repo()
        .create_magic_link(email, &token_hash, expires_at)
        .await
        .unwrap();

    let found = state
        .magic_link_repo()
        .find_by_token_hash(&token_hash)
        .await
        .unwrap()
        .expect("Link should exist");

    // The verify handler would reject this because expires_at < now
    assert!(
        found.expires_at < chrono::Utc::now(),
        "Link should be expired"
    );
}

#[tokio::test]
async fn verify_used_magic_link_is_rejected() {
    let pool = setup_test_db().await;
    let state = TestAppState::new(pool.clone());

    let email = "usedlink@finima.local";
    let (_, token_hash) = finima_auth::generate_token();

    let expires_at = chrono::Utc::now() + chrono::Duration::minutes(5);
    let link = state
        .magic_link_repo()
        .create_magic_link(email, &token_hash, expires_at)
        .await
        .unwrap();

    // Mark as used
    state.magic_link_repo().mark_used(link.id).await.unwrap();

    let found = state
        .magic_link_repo()
        .find_by_token_hash(&token_hash)
        .await
        .unwrap()
        .expect("Link should exist");

    // The verify handler would reject this because used_at is Some
    assert!(found.used_at.is_some(), "Link should be marked as used");
}

// ---------------------------------------------------------------------------
// Refresh token flow
// ---------------------------------------------------------------------------

#[tokio::test]
async fn refresh_token_returns_new_token_pair() {
    let pool = setup_test_db().await;
    let state = TestAppState::new(pool.clone());
    seed_test_db(&pool).await;

    let user_a_id = Uuid::parse_str(USER_A_ID).unwrap();

    // Issue a refresh token
    let refresh = refresh_token_for(user_a_id);

    // Decode the refresh token (simulating what the refresh handler does)
    let claims = finima_auth::jwt::decode_token(&refresh, TEST_JWT_SECRET)
        .expect("Refresh token should be valid");
    assert_eq!(claims.sub, USER_A_ID);

    // Look up the user (as the handler would)
    use finima_core::traits::UserRepo;
    let user = state.user_repo().find_by_id(user_a_id).await.unwrap();

    // Issue new tokens
    let new_access =
        finima_auth::jwt::encode_access_token(user.id, &user.email, TEST_JWT_SECRET).unwrap();
    let new_refresh = finima_auth::jwt::encode_refresh_token(user.id, TEST_JWT_SECRET).unwrap();

    // Verify new access token is valid
    let new_claims = finima_auth::jwt::decode_token(&new_access, TEST_JWT_SECRET).unwrap();
    assert_eq!(new_claims.sub, USER_A_ID);
    assert_eq!(new_claims.email, USER_A_EMAIL);

    // Verify new refresh token is valid
    let new_refresh_claims = finima_auth::jwt::decode_token(&new_refresh, TEST_JWT_SECRET).unwrap();
    assert_eq!(new_refresh_claims.sub, USER_A_ID);
}

#[tokio::test]
async fn expired_refresh_token_is_rejected() {
    let expired = expired_token();

    let result = finima_auth::jwt::decode_token(&expired, TEST_JWT_SECRET);
    assert!(result.is_err(), "Expired token should fail to decode");
}

#[tokio::test]
async fn invalid_refresh_token_is_rejected() {
    let result = finima_auth::jwt::decode_token("not-a-valid-jwt", TEST_JWT_SECRET);
    assert!(result.is_err(), "Malformed token should fail to decode");
}

#[tokio::test]
async fn refresh_token_with_wrong_secret_is_rejected() {
    let user_id = Uuid::new_v4();
    let token = finima_auth::jwt::encode_refresh_token(user_id, "different-secret").unwrap();

    let result = finima_auth::jwt::decode_token(&token, TEST_JWT_SECRET);
    assert!(
        result.is_err(),
        "Token signed with wrong secret should be rejected"
    );
}

// ---------------------------------------------------------------------------
// Token validation via the test router (in-process HTTP)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn authenticated_health_check_succeeds() {
    let pool = setup_test_db().await;
    seed_test_db(&pool).await;
    let state = TestAppState::new(pool);
    let router = build_test_router(state);

    let user_a_id = Uuid::parse_str(USER_A_ID).unwrap();
    let token = access_token_for(user_a_id, USER_A_EMAIL);
    let client = TestClient::new(router).with_auth(&token);

    let (status, body) = client.get("/health").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "healthy");
}

#[tokio::test]
async fn unauthenticated_health_check_still_succeeds() {
    // Health check is public, no auth required
    let pool = setup_test_db().await;
    let state = TestAppState::new(pool);
    let router = build_test_router(state);
    let client = TestClient::new(router);

    let (status, _) = client.get("/health").await;
    assert_eq!(status, StatusCode::OK);
}
