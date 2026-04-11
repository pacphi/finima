use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::{ConnectInfo, MatchedPath};
use axum::http::{HeaderValue, StatusCode};
use axum::middleware;
use axum::response::IntoResponse;
use axum::routing::{delete, get, post, put};
use axum::{Extension, Json, Router};
use serde_json::json;
use tokio::sync::Mutex;
use tower::ServiceBuilder;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::trace::TraceLayer;

use finima_auth::middleware::JwtSecret;

use crate::config::AppConfig;
use crate::handlers::{
    accounts, auth, budgets, dashboard, feed, flows, overrides, portfolios, recurring, savings,
    transactions, uploads, users,
};
use crate::metrics::{HttpDurationLabels, HttpRequestLabels, MetricsRegistry};
use crate::state::AppState;
use crate::ws;

// ---------------------------------------------------------------------------
// In-memory rate limiter for magic-link endpoint
// ---------------------------------------------------------------------------

/// Simple in-memory rate limiter: tracks request timestamps per IP address.
#[derive(Clone)]
struct RateLimiter {
    /// Map from IP address to list of request timestamps within the current window.
    requests: Arc<Mutex<HashMap<IpAddr, Vec<Instant>>>>,
    /// Maximum number of requests allowed within the window.
    max_requests: usize,
    /// Sliding window duration.
    window: Duration,
}

impl RateLimiter {
    fn new(max_requests: usize, window: Duration) -> Self {
        Self {
            requests: Arc::new(Mutex::new(HashMap::new())),
            max_requests,
            window,
        }
    }

    /// Returns `true` if the request is allowed, `false` if rate-limited.
    ///
    /// Also evicts entries for IPs whose timestamps have all expired, preventing
    /// unbounded memory growth from one-off visitors.
    async fn check(&self, ip: IpAddr) -> bool {
        let mut map = self.requests.lock().await;
        let now = Instant::now();

        // Evict IPs whose entries have all expired to prevent memory leaks.
        map.retain(|_ip, timestamps| {
            timestamps.retain(|t| now.duration_since(*t) < self.window);
            !timestamps.is_empty()
        });

        let timestamps = map.entry(ip).or_default();

        if timestamps.len() >= self.max_requests {
            false
        } else {
            timestamps.push(now);
            true
        }
    }
}

// ---------------------------------------------------------------------------
// Health check handler
// ---------------------------------------------------------------------------

/// GET /health — returns 200 with DB connectivity check.
///
/// Returns 200 OK when the database is reachable, 503 Service Unavailable
/// otherwise. Placed outside `/api/` so load balancers can probe it without
/// authentication.
async fn health_check(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> impl IntoResponse {
    match sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(state.pool())
        .await
    {
        Ok(_) => (
            StatusCode::OK,
            Json(json!({
                "status": "healthy",
                "version": env!("CARGO_PKG_VERSION"),
                "db": "ok"
            })),
        ),
        Err(e) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({
                "status": "unhealthy",
                "version": env!("CARGO_PKG_VERSION"),
                "db": format!("error: {e}")
            })),
        ),
    }
}

/// Axum middleware layer that injects the JWT secret into request extensions
/// so the `AuthUser` extractor can find it.
async fn inject_jwt_secret(
    axum::extract::State(state): axum::extract::State<AppState>,
    mut request: axum::http::Request<axum::body::Body>,
    next: middleware::Next,
) -> axum::response::Response {
    request
        .extensions_mut()
        .insert(JwtSecret(state.config().auth.jwt_secret.clone()));
    next.run(request).await
}

/// Middleware that enforces rate limiting using the `RateLimiter` stored as an
/// Axum extension. Extracts the client IP from `ConnectInfo` if available,
/// otherwise falls back to 0.0.0.0.
async fn rate_limit_middleware(
    limiter: axum::Extension<RateLimiter>,
    request: axum::http::Request<axum::body::Body>,
    next: middleware::Next,
) -> axum::response::Response {
    let ip = request
        .extensions()
        .get::<ConnectInfo<std::net::SocketAddr>>()
        .map(|ci| ci.0.ip())
        .unwrap_or(IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED));

    if !limiter.check(ip).await {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({"error": "Rate limit exceeded. Try again later."})),
        )
            .into_response();
    }

    next.run(request).await
}

// ---------------------------------------------------------------------------
// Prometheus metrics middleware
// ---------------------------------------------------------------------------

/// Axum middleware that records HTTP request count, duration, in-flight gauge,
/// and 5xx error counter for every request passing through the router.
///
/// Uses `MatchedPath` to record the route template (e.g. `/api/users/{id}`)
/// rather than the concrete path, keeping metric cardinality bounded.
async fn metrics_middleware(
    metrics_registry: Extension<MetricsRegistry>,
    matched_path: Option<MatchedPath>,
    request: axum::http::Request<axum::body::Body>,
    next: middleware::Next,
) -> axum::response::Response {
    let method = request.method().to_string();
    let path = matched_path
        .map(|mp| mp.as_str().to_owned())
        .unwrap_or_else(|| request.uri().path().to_owned());

    let m = metrics_registry.metrics();

    // Track in-flight requests.
    m.http_requests_in_flight.inc();
    let start = Instant::now();

    let response = next.run(request).await;

    let elapsed = start.elapsed().as_secs_f64();
    let status = response.status().as_u16();

    // Record request counter.
    m.http_requests_total
        .get_or_create(&HttpRequestLabels {
            method: method.clone(),
            path: path.clone(),
            status_code: status,
        })
        .inc();

    // Record duration histogram.
    m.http_request_duration_seconds
        .get_or_create(&HttpDurationLabels { method, path })
        .observe(elapsed);

    // Track 5xx errors for error budget.
    if status >= 500 {
        m.error_rate_5xx.inc();
    }

    // Decrement in-flight gauge.
    m.http_requests_in_flight.dec();

    response
}

// ---------------------------------------------------------------------------
// Metrics exposition endpoint
// ---------------------------------------------------------------------------

/// GET /metrics — returns all registered Prometheus metrics in text exposition
/// format. Unauthenticated, like /health.
async fn metrics_handler(metrics_registry: Extension<MetricsRegistry>) -> impl IntoResponse {
    let body = metrics_registry.render();
    (
        StatusCode::OK,
        [(
            http::header::CONTENT_TYPE,
            HeaderValue::from_static("text/plain; version=0.0.4; charset=utf-8"),
        )],
        body,
    )
}

/// Build the complete Axum router with all route groups and middleware layers.
pub fn build_router(
    state: AppState,
    config: &AppConfig,
    metrics_registry: MetricsRegistry,
) -> Router {
    // Rate limiter for magic-link endpoint: 5 requests per minute per IP.
    // Stored in AppState extensions so the rate-limit middleware can access it.
    let magic_link_limiter = RateLimiter::new(5, Duration::from_secs(60));

    // Auth routes (no authentication required).
    // The magic-link route gets a rate-limiting middleware layer.
    let magic_link_route = Router::new()
        .route("/magic-link", post(auth::request_magic_link))
        .layer(axum::Extension(magic_link_limiter))
        .layer(middleware::from_fn(rate_limit_middleware));

    let auth_routes = magic_link_route
        .route("/verify", post(auth::verify_magic_link))
        .route("/refresh", post(auth::refresh_token))
        .route("/session", delete(auth::delete_session));

    // Portfolio routes (authentication required)
    let portfolio_routes = Router::new()
        .route(
            "/",
            get(portfolios::list_portfolios).post(portfolios::create_portfolio),
        )
        .route(
            "/{id}",
            get(portfolios::get_portfolio).put(portfolios::update_portfolio),
        );

    // Account routes (authentication required)
    let account_routes = Router::new()
        .route(
            "/",
            get(accounts::list_accounts).post(accounts::create_account),
        )
        .route(
            "/{id}",
            get(accounts::get_account)
                .put(accounts::update_account)
                .delete(accounts::delete_account),
        );

    // Upload routes (authentication required) — 50 MB body limit for file uploads
    let upload_routes = Router::new()
        .route("/", post(uploads::create_upload))
        .route("/{id}/preview", get(uploads::get_preview))
        .route("/{id}/confirm", post(uploads::confirm_upload))
        .route("/{id}/status", get(uploads::get_upload_status))
        .layer(RequestBodyLimitLayer::new(50 * 1024 * 1024)); // 50 MB

    // Transaction routes (authentication required)
    let transaction_routes = Router::new()
        .route("/", get(transactions::list_transactions))
        .route("/search", get(transactions::search_transactions))
        .route("/bulk-update", post(transactions::bulk_update_transactions))
        .route("/{id}", put(transactions::update_transaction));

    // Recurring routes (authentication required)
    let recurring_routes = Router::new()
        .route("/", get(recurring::list_recurring))
        .route("/{id}", put(recurring::update_recurring));

    // User override routes (authentication required)
    let override_routes = Router::new().route(
        "/",
        get(overrides::list_overrides).post(overrides::create_override),
    );

    // Feed routes (authentication required)
    let feed_routes = Router::new()
        .route("/", get(feed::list_feed))
        .route("/{id}/summary", get(feed::get_article_summary));

    // User routes (authentication required)
    let user_routes = Router::new()
        .route("/me", get(users::get_current_user))
        .route("/me/preferences", put(users::update_preferences));

    // Dashboard routes (authentication required)
    let dashboard_routes = Router::new()
        .route("/summary", get(dashboard::get_summary))
        .route("/net-worth", get(dashboard::get_net_worth))
        .route("/cashflow", get(dashboard::get_cashflow))
        .route("/spending", get(dashboard::get_spending));

    // Budget routes (authentication required)
    let budget_routes = Router::new()
        .route("/", get(budgets::list_budgets).post(budgets::create_budget))
        .route("/vs-actual", get(budgets::budget_vs_actual))
        .route("/auto-suggest", post(budgets::auto_suggest));

    // Savings goal routes (authentication required)
    let savings_routes = Router::new()
        .route("/", get(savings::list_goals).post(savings::create_goal))
        .route(
            "/{id}",
            put(savings::update_goal).delete(savings::delete_goal),
        );

    // Flow routes (authentication required)
    let flow_routes = Router::new()
        .route("/", get(flows::list_flows).post(flows::create_flow))
        .route("/sankey", get(flows::get_sankey))
        .route("/outflow-ranking", get(flows::get_outflow_ranking))
        .route("/balance-impact", get(flows::get_balance_impact))
        .route("/{id}", put(flows::update_flow).delete(flows::delete_flow));

    // Flow group routes (authentication required)
    let flow_group_routes = Router::new()
        .route(
            "/",
            get(flows::list_flow_groups).post(flows::create_flow_group),
        )
        .route(
            "/{id}",
            put(flows::update_flow_group).delete(flows::delete_flow_group),
        );

    // Build CORS layer from config
    let cors = build_cors_layer(config);

    // Determine if we should send HSTS header (production only)
    let app_env = std::env::var("APP_ENV").unwrap_or_default();
    let hsts_value = if app_env == "production" {
        "max-age=63072000; includeSubDomains; preload"
    } else {
        "max-age=0"
    };

    Router::new()
        // Health check — no auth required, at root for load balancers
        .route("/health", get(health_check))
        // Prometheus metrics — no auth required, for scraping
        .route("/metrics", get(metrics_handler))
        .nest("/api/auth", auth_routes)
        .nest("/api/portfolios", portfolio_routes)
        .nest("/api/accounts", account_routes)
        .nest("/api/uploads", upload_routes)
        .nest("/api/transactions", transaction_routes)
        .nest("/api/recurring", recurring_routes)
        .nest("/api/user-overrides", override_routes)
        .nest("/api/feed", feed_routes)
        .nest("/api/users", user_routes)
        .nest("/api/dashboard", dashboard_routes)
        .nest("/api/budgets", budget_routes)
        .nest("/api/savings-goals", savings_routes)
        .nest("/api/flows", flow_routes)
        .nest("/api/flow-groups", flow_group_routes)
        // WebSocket route — auth via query param, not middleware
        .route("/api/ws", get(ws::ws_handler))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            inject_jwt_secret,
        ))
        .layer(Extension(metrics_registry))
        .layer(middleware::from_fn(metrics_middleware))
        .layer(
            ServiceBuilder::new()
                // Security headers
                .layer(SetResponseHeaderLayer::overriding(
                    http::header::HeaderName::from_static("x-content-type-options"),
                    HeaderValue::from_static("nosniff"),
                ))
                .layer(SetResponseHeaderLayer::overriding(
                    http::header::HeaderName::from_static("x-frame-options"),
                    HeaderValue::from_static("DENY"),
                ))
                .layer(SetResponseHeaderLayer::overriding(
                    http::header::STRICT_TRANSPORT_SECURITY,
                    HeaderValue::from_str(hsts_value).unwrap(),
                ))
                // Default body limit: 1 MB (upload routes override with 50 MB)
                .layer(RequestBodyLimitLayer::new(1024 * 1024))
                .layer(TraceLayer::new_for_http())
                .layer(cors),
        )
        .with_state(state)
}

/// Build a CORS layer from the application config.
///
/// In production, an empty `allowed_origins` list is treated as a misconfiguration
/// and causes a panic to prevent a fail-open CORS policy. In non-production
/// environments, an empty list falls back to `AllowOrigin::any()` for convenience.
fn build_cors_layer(config: &AppConfig) -> CorsLayer {
    let origins = &config.cors.allowed_origins;
    let app_env = std::env::var("APP_ENV").unwrap_or_default();

    let allow_origin = if origins.is_empty() {
        if app_env == "production" {
            panic!("FATAL: CORS allowed_origins must be configured in production");
        }
        AllowOrigin::any()
    } else {
        let parsed: Vec<_> = origins.iter().filter_map(|o| o.parse().ok()).collect();
        AllowOrigin::list(parsed)
    };

    CorsLayer::new()
        .allow_origin(allow_origin)
        .allow_methods([
            http::Method::GET,
            http::Method::POST,
            http::Method::PUT,
            http::Method::DELETE,
            http::Method::OPTIONS,
        ])
        .allow_headers([http::header::CONTENT_TYPE, http::header::AUTHORIZATION])
}
