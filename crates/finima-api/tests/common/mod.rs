#![allow(dead_code, unused_imports)]
//! Shared test infrastructure for finima-api integration tests.
//!
//! Provides helpers to:
//! - Build a fully-wired Axum router pointing at the test database.
//! - Create authenticated test users and obtain JWT tokens.
//! - Run migrations and seed data against the test database.
//!
//! Because `finima-api` is a binary crate (no `lib.rs`), the integration tests
//! reconstruct the router from the public library crates (`finima-db`,
//! `finima-auth`, `finima-core`, etc.) rather than importing from `finima-api`.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{self, Request, StatusCode};
use axum::middleware;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use tower_http::cors::{AllowOrigin, CorsLayer};

/// Minimal LLM client for tests — returns errors, never called in unit tests.
struct NoOpLlmClient;

#[async_trait::async_trait]
impl finima_llm::LlmClient for NoOpLlmClient {
    async fn categorize_batch(
        &self,
        _batch: &finima_llm::CategorizationBatch,
    ) -> Result<Vec<finima_llm::CategorizationResult>, finima_llm::LlmError> {
        Err(finima_llm::LlmError::Configuration(
            "No LLM configured in test".to_string(),
        ))
    }

    async fn enrich_recurring(
        &self,
        _group: &finima_llm::RecurringGroupCandidate,
    ) -> Result<finima_llm::RecurringEnrichment, finima_llm::LlmError> {
        Err(finima_llm::LlmError::Configuration(
            "No LLM configured in test".to_string(),
        ))
    }

    async fn generate_insight(&self, _prompt: &str) -> Result<String, finima_llm::LlmError> {
        Err(finima_llm::LlmError::Configuration(
            "No LLM configured in test".to_string(),
        ))
    }
}
use tower_http::trace::TraceLayer;
use uuid::Uuid;

use finima_auth::middleware::JwtSecret;
use finima_auth::{jwt, LoggingEmailSender};

// ---------------------------------------------------------------------------
// Test configuration (matches config/test.yaml)
// ---------------------------------------------------------------------------

/// Database URL for integration tests.
/// Override with the `TEST_DATABASE_URL` environment variable.
fn test_database_url() -> String {
    std::env::var("TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://finima:test@localhost:5433/finima_test".to_string())
}

/// JWT secret used across all integration tests.
pub const TEST_JWT_SECRET: &str = "test-secret-do-not-use-in-production";

// ---------------------------------------------------------------------------
// AppState — minimal recreation for testing
// ---------------------------------------------------------------------------

/// Application state mirroring the real `AppState` in `finima-api`.
///
/// We reconstruct this here because the binary crate does not export it.
/// It carries the same pool, repos, and config needed by the handlers.
#[derive(Clone)]
pub struct TestAppState {
    inner: Arc<TestInnerState>,
}

struct TestInnerState {
    pool: PgPool,
    jwt_secret: String,
    email_sender: Box<dyn finima_auth::EmailSender>,
    user_repo: finima_db::PgUserRepo,
    portfolio_repo: finima_db::PgPortfolioRepo,
    account_repo: finima_db::PgAccountRepo,
    magic_link_repo: finima_db::PgMagicLinkRepo,
    transaction_repo: finima_db::PgTransactionRepo,
    upload_repo: finima_db::PgUploadRepo,
    recurring_repo: finima_db::PgRecurringRepo,
    override_repo: finima_db::PgOverrideRepo,
    budget_repo: finima_db::PgBudgetRepo,
    savings_goal_repo: finima_db::PgSavingsGoalRepo,
    flow_repo: finima_db::PgFlowRepo,
    flow_group_repo: finima_db::PgFlowGroupRepo,
    flow_pattern_repo: finima_db::repos::FlowPatternRepo,
    embedding_index_repo: finima_db::repos::EmbeddingIndexRepo,
    llm_client: Arc<dyn finima_llm::LlmClient>,
}

impl TestAppState {
    pub fn new(pool: PgPool) -> Self {
        Self {
            inner: Arc::new(TestInnerState {
                user_repo: finima_db::PgUserRepo::new(pool.clone()),
                portfolio_repo: finima_db::PgPortfolioRepo::new(pool.clone()),
                account_repo: finima_db::PgAccountRepo::new(pool.clone()),
                magic_link_repo: finima_db::PgMagicLinkRepo::new(pool.clone()),
                transaction_repo: finima_db::PgTransactionRepo::new(pool.clone()),
                upload_repo: finima_db::PgUploadRepo::new(pool.clone()),
                recurring_repo: finima_db::PgRecurringRepo::new(pool.clone()),
                override_repo: finima_db::PgOverrideRepo::new(pool.clone()),
                budget_repo: finima_db::PgBudgetRepo::new(pool.clone()),
                savings_goal_repo: finima_db::PgSavingsGoalRepo::new(pool.clone()),
                flow_repo: finima_db::PgFlowRepo::new(pool.clone()),
                flow_group_repo: finima_db::PgFlowGroupRepo::new(pool.clone()),
                flow_pattern_repo: finima_db::repos::FlowPatternRepo::new(pool.clone()),
                embedding_index_repo: finima_db::repos::EmbeddingIndexRepo::new(pool.clone()),
                email_sender: Box::new(LoggingEmailSender),
                jwt_secret: TEST_JWT_SECRET.to_string(),
                llm_client: Arc::new(NoOpLlmClient),
                pool,
            }),
        }
    }

    pub fn pool(&self) -> &PgPool {
        &self.inner.pool
    }

    pub fn jwt_secret(&self) -> &str {
        &self.inner.jwt_secret
    }

    pub fn user_repo(&self) -> &finima_db::PgUserRepo {
        &self.inner.user_repo
    }

    pub fn magic_link_repo(&self) -> &finima_db::PgMagicLinkRepo {
        &self.inner.magic_link_repo
    }

    pub fn portfolio_repo(&self) -> &finima_db::PgPortfolioRepo {
        &self.inner.portfolio_repo
    }

    pub fn account_repo(&self) -> &finima_db::PgAccountRepo {
        &self.inner.account_repo
    }

    pub fn savings_goal_repo(&self) -> &finima_db::PgSavingsGoalRepo {
        &self.inner.savings_goal_repo
    }

    pub fn transaction_repo(&self) -> &finima_db::PgTransactionRepo {
        &self.inner.transaction_repo
    }

    pub fn flow_repo(&self) -> &finima_db::PgFlowRepo {
        &self.inner.flow_repo
    }

    pub fn flow_pattern_repo(&self) -> &finima_db::repos::FlowPatternRepo {
        &self.inner.flow_pattern_repo
    }

    pub fn embedding_index_repo(&self) -> &finima_db::repos::EmbeddingIndexRepo {
        &self.inner.embedding_index_repo
    }
}

// ---------------------------------------------------------------------------
// Database setup
// ---------------------------------------------------------------------------

/// Create a connection pool to the test database and run migrations.
///
/// Each call creates a fresh pool. Migrations are idempotent so running
/// them multiple times is safe.
pub async fn setup_test_db() -> PgPool {
    let pool = finima_db::create_pool(&test_database_url(), 5)
        .await
        .expect("Failed to connect to test database. Is docker-compose.test.yml running?");

    sqlx::migrate!("../finima-db/src/migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations on test database");

    pool
}

/// Seed the test database with deterministic fixture data from `tests/seed.sql`.
///
/// Uses `ON CONFLICT DO NOTHING` so it is safe to call repeatedly.
pub async fn seed_test_db(pool: &PgPool) {
    let seed_sql = include_str!("../../../../tests/seed.sql");
    sqlx::raw_sql(seed_sql)
        .execute(pool)
        .await
        .expect("Failed to execute seed.sql");
}

/// Create a second test user (User B) for cross-user authorization tests.
///
/// Returns the user's UUID. The user is created directly in the database
/// with a deterministic ID so assertions are predictable.
pub async fn create_user_b(pool: &PgPool) -> Uuid {
    let user_b_id = Uuid::parse_str("a2000000-0000-4000-8000-000000000002").unwrap();
    sqlx::query(
        r#"
        INSERT INTO users (id, email, display_name, created_at, updated_at)
        VALUES ($1, 'userb@finima.local', 'User B', NOW(), NOW())
        ON CONFLICT (id) DO NOTHING
        "#,
    )
    .bind(user_b_id)
    .execute(pool)
    .await
    .expect("Failed to create User B");

    // Give User B their own portfolio
    let portfolio_b_id = Uuid::parse_str("b2000000-0000-4000-8000-000000000002").unwrap();
    sqlx::query(
        r#"
        INSERT INTO portfolios (id, user_id, name, created_at)
        VALUES ($1, $2, 'User B Portfolio', NOW())
        ON CONFLICT (id) DO NOTHING
        "#,
    )
    .bind(portfolio_b_id)
    .bind(user_b_id)
    .execute(pool)
    .await
    .expect("Failed to create User B portfolio");

    // Give User B a savings goal
    sqlx::query(
        r#"
        INSERT INTO savings_goals (id, portfolio_id, name, target_amount, current_amount)
        VALUES ($1, $2, 'User B Goal', 5000.00, 1000.00)
        ON CONFLICT (id) DO NOTHING
        "#,
    )
    .bind(Uuid::parse_str("e2000000-0000-4000-8000-000000000001").unwrap())
    .bind(portfolio_b_id)
    .execute(pool)
    .await
    .expect("Failed to create User B savings goal");

    user_b_id
}

// ---------------------------------------------------------------------------
// JWT helpers
// ---------------------------------------------------------------------------

/// Deterministic User A ID from seed.sql.
pub const USER_A_ID: &str = "a1000000-0000-4000-8000-000000000001";
pub const USER_A_EMAIL: &str = "test@finima.local";

/// Generate a valid JWT access token for the given user.
pub fn access_token_for(user_id: Uuid, email: &str) -> String {
    jwt::encode_access_token(user_id, email, TEST_JWT_SECRET)
        .expect("Failed to encode test access token")
}

/// Generate a valid JWT refresh token for the given user.
pub fn refresh_token_for(user_id: Uuid) -> String {
    jwt::encode_refresh_token(user_id, TEST_JWT_SECRET)
        .expect("Failed to encode test refresh token")
}

/// Generate an expired JWT token for testing rejection.
pub fn expired_token() -> String {
    use jsonwebtoken::{encode, EncodingKey, Header};
    let now = chrono::Utc::now().timestamp() as usize;
    let claims = jwt::Claims {
        sub: Uuid::new_v4().to_string(),
        email: "expired@test.local".to_string(),
        exp: now - 3600, // expired 1 hour ago
        iat: now - 7200,
        token_type: finima_auth::jwt::TokenType::Access,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(TEST_JWT_SECRET.as_bytes()),
    )
    .unwrap()
}

// ---------------------------------------------------------------------------
// Router construction — mirrors build_router from finima-api/src/router.rs
// ---------------------------------------------------------------------------

/// Middleware that injects the JWT secret into request extensions.
async fn inject_jwt_secret(
    axum::extract::State(state): axum::extract::State<TestAppState>,
    mut request: Request<Body>,
    next: middleware::Next,
) -> axum::response::Response {
    request
        .extensions_mut()
        .insert(JwtSecret(state.jwt_secret().to_string()));
    next.run(request).await
}

/// Health check handler for the test router.
async fn health_check(
    axum::extract::State(state): axum::extract::State<TestAppState>,
) -> impl IntoResponse {
    match sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(state.pool())
        .await
    {
        Ok(_) => (StatusCode::OK, Json(json!({"status": "healthy"}))),
        Err(e) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"status": "unhealthy", "error": e.to_string()})),
        ),
    }
}

/// Build a minimal test router with the health and auth endpoints.
///
/// This is intentionally a subset of the full production router. We include
/// only the routes needed by the integration tests to keep compilation fast
/// and test surface focused. Add more route groups here as new test files
/// need them.
///
/// NOTE: The production `finima-api` binary crate owns the real handlers.
/// Because those are not accessible from integration tests (binary crate,
/// no `lib.rs`), this test router wires up only the health endpoint and
/// auth-adjacent routes that can be exercised without the real handlers.
///
/// For tests that need to exercise the *real* handler logic, prefer the
/// `TestClient` which builds requests against the real server if one is
/// running, or use the approach of testing the handler functions via
/// the real `finima-api` binary by starting it in the background.
///
/// However, for the initial integration test suite we take a pragmatic
/// approach: we test what we can via the library crates directly (auth
/// token generation/verification, DB operations, authorization checks)
/// and use the health endpoint as a smoke test for the in-process router
/// pattern.
pub fn build_test_router(state: TestAppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::any())
        .allow_methods([
            http::Method::GET,
            http::Method::POST,
            http::Method::PUT,
            http::Method::DELETE,
        ])
        .allow_headers([http::header::CONTENT_TYPE, http::header::AUTHORIZATION]);

    Router::new()
        .route("/health", get(health_check))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            inject_jwt_secret,
        ))
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state)
}

// ---------------------------------------------------------------------------
// TestClient — ergonomic wrapper for in-process HTTP requests
// ---------------------------------------------------------------------------

/// A lightweight HTTP client for in-process integration testing.
///
/// Wraps an `axum::Router` and uses `tower::ServiceExt::oneshot` to send
/// requests without binding a TCP port. Supports setting a default
/// Authorization header for authenticated requests.
pub struct TestClient {
    router: Router,
    auth_token: Option<String>,
}

impl TestClient {
    /// Create a new `TestClient` wrapping the given router.
    pub fn new(router: Router) -> Self {
        Self {
            router,
            auth_token: None,
        }
    }

    /// Set the Bearer token used for all subsequent requests.
    pub fn with_auth(mut self, token: &str) -> Self {
        self.auth_token = Some(token.to_string());
        self
    }

    /// Send a GET request and return the status code and deserialized body.
    pub async fn get(&self, uri: &str) -> (StatusCode, Value) {
        let mut builder = Request::builder().method("GET").uri(uri);
        if let Some(ref token) = self.auth_token {
            builder = builder.header("Authorization", format!("Bearer {}", token));
        }
        let request = builder.body(Body::empty()).unwrap();
        self.send(request).await
    }

    /// Send a POST request with a JSON body.
    pub async fn post(&self, uri: &str, body: &Value) -> (StatusCode, Value) {
        let body_bytes = serde_json::to_vec(body).unwrap();
        let mut builder = Request::builder()
            .method("POST")
            .uri(uri)
            .header("Content-Type", "application/json");
        if let Some(ref token) = self.auth_token {
            builder = builder.header("Authorization", format!("Bearer {}", token));
        }
        let request = builder.body(Body::from(body_bytes)).unwrap();
        self.send(request).await
    }

    /// Send a PUT request with a JSON body.
    pub async fn put(&self, uri: &str, body: &Value) -> (StatusCode, Value) {
        let body_bytes = serde_json::to_vec(body).unwrap();
        let mut builder = Request::builder()
            .method("PUT")
            .uri(uri)
            .header("Content-Type", "application/json");
        if let Some(ref token) = self.auth_token {
            builder = builder.header("Authorization", format!("Bearer {}", token));
        }
        let request = builder.body(Body::from(body_bytes)).unwrap();
        self.send(request).await
    }

    /// Send a DELETE request.
    pub async fn delete(&self, uri: &str) -> (StatusCode, Value) {
        let mut builder = Request::builder().method("DELETE").uri(uri);
        if let Some(ref token) = self.auth_token {
            builder = builder.header("Authorization", format!("Bearer {}", token));
        }
        let request = builder.body(Body::empty()).unwrap();
        self.send(request).await
    }

    /// Internal: dispatch a request through the router and collect the response.
    async fn send(&self, request: Request<Body>) -> (StatusCode, Value) {
        let response = self
            .router
            .clone()
            .oneshot(request)
            .await
            .expect("Failed to send request through test router");

        let status = response.status();
        let body_bytes = response
            .into_body()
            .collect()
            .await
            .expect("Failed to read response body")
            .to_bytes();

        let body: Value = if body_bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&body_bytes).unwrap_or(Value::String(
                String::from_utf8_lossy(&body_bytes).to_string(),
            ))
        };

        (status, body)
    }
}

// ---------------------------------------------------------------------------
// Full auth flow helper — creates a magic link and verifies it
// ---------------------------------------------------------------------------

/// Perform the full magic link authentication flow for the given email
/// directly against the database (bypassing HTTP handlers).
///
/// Returns `(user_id, access_token, refresh_token)`.
///
/// This is useful for tests that need an authenticated user but do not
/// want to exercise the auth endpoints themselves.
pub async fn authenticate_user(state: &TestAppState, email: &str) -> (Uuid, String, String) {
    use finima_core::traits::UserRepo;

    // Find or create the user
    let user = match state.user_repo().find_by_email(email).await.unwrap() {
        Some(u) => u,
        None => {
            let display_name = email.split('@').next().unwrap_or("Test").to_string();
            state
                .user_repo()
                .create_user(email, &display_name)
                .await
                .unwrap()
        }
    };

    let access = access_token_for(user.id, &user.email);
    let refresh = refresh_token_for(user.id);

    (user.id, access, refresh)
}
