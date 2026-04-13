mod config;
mod error_response;
mod handlers;
mod metrics;
mod router;
mod state;
mod storage;
mod ws;

use tracing_subscriber::EnvFilter;

use finima_auth::{LoggingEmailSender, ResendClient};

#[tokio::main]
async fn main() {
    // Load configuration
    let app_config = config::load_config().expect("Failed to load configuration");

    // Set up tracing subscriber based on config
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(&app_config.logging.level));

    match app_config.logging.format.as_str() {
        "json" => {
            tracing_subscriber::fmt()
                .json()
                .with_env_filter(filter)
                .init();
        }
        _ => {
            tracing_subscriber::fmt()
                .pretty()
                .with_env_filter(filter)
                .init();
        }
    }

    tracing::info!("Finima API server starting...");
    tracing::info!(
        host = %app_config.server.host,
        port = %app_config.server.port,
        "Server configuration loaded"
    );

    // Create database connection pool
    let pool = finima_db::create_pool(
        &app_config.database.resolved_url(),
        app_config.database.max_connections,
    )
    .await
    .expect("Failed to create database pool");

    tracing::info!("Database pool created");

    // Run SQLx migrations
    sqlx::migrate!("../finima-db/src/migrations")
        .run(&pool)
        .await
        .expect("Failed to run database migrations");

    tracing::info!("Database migrations applied");

    // Create email sender based on environment
    let app_env = std::env::var("APP_ENV").unwrap_or_else(|_| "development".to_string());
    let email_sender: Box<dyn finima_auth::EmailSender> = match app_env.as_str() {
        "production" => {
            tracing::info!("Using Resend email sender for production");
            Box::new(ResendClient::new(
                app_config.resend.api_key.clone(),
                app_config.auth.from_email.clone(),
            ))
        }
        _ => {
            tracing::info!("Using logging email sender for development/test");
            Box::new(LoggingEmailSender)
        }
    };

    // Initialize S3-compatible object storage
    let object_storage = storage::ObjectStorage::new(&app_config.s3)
        .await
        .expect("Failed to initialize object storage");

    tracing::info!(
        endpoint = %app_config.s3.endpoint_url,
        bucket = %app_config.s3.bucket,
        "Object storage initialized"
    );

    // Build application state
    let state = state::AppState::new(pool, app_config.clone(), email_sender, object_storage).await;

    // Initialize Prometheus metrics registry
    let metrics_registry = metrics::MetricsRegistry::new();
    tracing::info!("Prometheus metrics registry initialized");

    // Build router
    let app = router::build_router(state, &app_config, metrics_registry);

    // Start server
    let bind_addr = format!("{}:{}", app_config.server.host, app_config.server.port);
    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .expect("Failed to bind to address");

    tracing::info!(address = %bind_addr, "Finima API server listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("Server error");
}

/// Wait for a Ctrl-C (SIGINT) signal, then log and return so `axum::serve`
/// can drain in-flight connections gracefully.
async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to listen for ctrl_c signal");
    tracing::info!("Shutting down gracefully...");
}
