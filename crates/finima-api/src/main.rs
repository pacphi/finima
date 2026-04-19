mod config;
mod error_response;
mod handlers;
mod metrics;
mod router;
mod state;
mod storage;
mod ws;

use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

use finima_auth::{LoggingEmailSender, ResendClient};
use finima_feed::{CachedFeedService, FeedFetcher, FeedSource};

#[tokio::main]
async fn main() {
    // Load .env file if present (silently ignored if missing).
    // This makes APP__* env vars available to the config crate regardless
    // of whether the user starts the app via Make, Docker, or cargo run.
    dotenvy::dotenv().ok();

    // Load configuration
    let app_config = config::load_config().expect("Failed to load configuration");

    // Set up tracing: console output + optional daily-rotating file log.
    // RUST_LOG env var takes precedence over the YAML-configured level.
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(&app_config.logging.level));

    // Console layer — ANSI colours when format is "pretty", JSON otherwise.
    let console_layer: Box<dyn tracing_subscriber::Layer<_> + Send + Sync> =
        match app_config.logging.format.as_str() {
            "json" => Box::new(tracing_subscriber::fmt::layer().json()),
            _ => Box::new(tracing_subscriber::fmt::layer().with_ansi(true)),
        };

    // Optional rolling-file layer (plain text, no ANSI escapes).
    // The _file_guard must live until main() returns to keep writes flushing.
    let _file_guard: Option<tracing_appender::non_blocking::WorkerGuard>;
    let file_layer: Option<Box<dyn tracing_subscriber::Layer<_> + Send + Sync>>;

    if !app_config.logging.log_dir.is_empty() {
        let log_dir = std::path::Path::new(&app_config.logging.log_dir);
        std::fs::create_dir_all(log_dir).expect("Failed to create log directory");

        let file_appender = tracing_appender::rolling::daily(log_dir, "finima.log");
        let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
        _file_guard = Some(guard);
        file_layer = Some(Box::new(
            tracing_subscriber::fmt::layer()
                .with_ansi(false)
                .with_writer(non_blocking),
        ));
    } else {
        _file_guard = None;
        file_layer = None;
    }

    tracing_subscriber::registry()
        .with(filter)
        .with(console_layer)
        .with(file_layer)
        .init();

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

    // Create email sender: use Resend when an API key is configured, otherwise log-only
    let app_env = std::env::var("APP_ENV").unwrap_or_else(|_| "development".to_string());
    let email_sender: Box<dyn finima_auth::EmailSender> = if !app_config.resend.api_key.is_empty() {
        tracing::info!(env = app_env, "Using Resend email sender");
        Box::new(ResendClient::new(
            app_config.resend.api_key.clone(),
            app_config.auth.from_email.clone(),
        ))
    } else {
        tracing::info!(
                env = app_env,
                "No Resend API key configured — using logging email sender (emails will NOT be delivered)"
            );
        Box::new(LoggingEmailSender)
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

    // Initialize cached feed service — starts empty, fetches in the background
    // so the server is responsive immediately. Refreshes every poll_interval.
    let feed_sources: Vec<FeedSource> = app_config
        .feed
        .sources
        .iter()
        .map(|s| FeedSource {
            name: s.name.clone(),
            url: s.url.clone(),
            topic: s.topic.clone(),
            enabled: true,
        })
        .collect();
    let feed_service = CachedFeedService::new(feed_sources, FeedFetcher::new());
    feed_service.start_background_refresh(std::time::Duration::from_secs(
        u64::from(app_config.feed.poll_interval_hours) * 3600,
    ));

    // Build application state (starts without an LLM client so the server
    // can accept requests immediately while the model loads in the background).
    let state = state::AppState::new(
        pool,
        app_config.clone(),
        email_sender,
        object_storage,
        feed_service,
    );

    // Spawn background LLM loading so the server is responsive during model init.
    spawn_llm_loader(state.clone(), &app_config);

    // Initialize Prometheus metrics registry
    let metrics_registry = metrics::MetricsRegistry::new();
    tracing::info!("Prometheus metrics registry initialized");

    // Share the metrics registry with AppState so handlers can record
    // Tier 2 / flow-pattern / bootstrap counters.
    state.set_metrics(metrics_registry.clone());

    // Build router
    let app = router::build_router(state.clone(), &app_config, metrics_registry);

    // Start server
    let bind_addr = format!("{}:{}", app_config.server.host, app_config.server.port);
    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .expect("Failed to bind to address");

    tracing::info!(address = %bind_addr, "Finima API server listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(state))
        .await
        .expect("Server error");
}

/// Spawn a background task that loads the configured LLM backend and swaps it
/// into `state` once ready. This keeps the server responsive during the
/// (potentially multi-minute) model download / quantization step.
fn spawn_llm_loader(state: state::AppState, config: &config::AppConfig) {
    use std::sync::Arc;

    let llm_config = config.llm.clone();
    tokio::spawn(async move {
        tracing::info!(provider = %llm_config.provider, "Loading LLM backend in background...");

        // If the provider is explicitly "none" or empty, skip LLM entirely.
        // Categorization will use Tiers 0-2 only.
        match llm_config.provider.as_str() {
            "none" | "" => {
                tracing::info!(
                    "LLM provider set to '{}' — categorization uses Tiers 0-2 only",
                    llm_config.provider
                );
                state.set_llm_disabled();
                return;
            }
            _ => {}
        }

        let result: Result<Arc<dyn finima_llm::LlmClient>, String> =
            match llm_config.provider.as_str() {
                #[cfg(feature = "candle")]
                "candle" => {
                    use finima_llm::{CandleClient, CandleConfig as LlmCandleConfig};
                    let candle_cfg = LlmCandleConfig {
                        model_id: llm_config.candle.model_id.clone(),
                        model_path: llm_config.candle.model_path.clone(),
                        quantization: llm_config.candle.quantization.clone(),
                        device: llm_config.candle.device.clone(),
                        context_length: llm_config.candle.context_length,
                        threads: llm_config.candle.threads,
                    };
                    CandleClient::new(candle_cfg)
                        .await
                        .map(|c| Arc::new(c) as Arc<dyn finima_llm::LlmClient>)
                        .map_err(|e| format!("Candle initialization failed: {e}"))
                }
                #[cfg(not(feature = "candle"))]
                "candle" => {
                    Err("Provider is 'candle' but the candle feature is not enabled".to_string())
                }
                #[cfg(feature = "ollama")]
                "ollama" if !llm_config.ollama.url.is_empty() => {
                    Ok(Arc::new(finima_llm::OllamaClient::with_config(
                        &llm_config.ollama.url,
                        &llm_config.ollama.model,
                        llm_config.timeout_seconds,
                        llm_config.max_retries,
                    )))
                }
                #[cfg(not(feature = "ollama"))]
                "ollama" => {
                    Err("Provider is 'ollama' but the ollama feature is not enabled".to_string())
                }
                other => Err(format!("Unknown LLM provider: '{other}'")),
            };

        match result {
            Ok(client) => {
                // Warm up the model so the first categorization request
                // doesn't pay cold-start latency (model loading into GPU).
                if let Err(e) = client.warmup().await {
                    tracing::warn!(error = %e, "LLM warmup failed (non-fatal)");
                }
                state.set_llm_client(client);
                tracing::info!("LLM backend loaded and ready");
            }
            Err(msg) => {
                tracing::error!(error = %msg, "LLM backend failed to load");
                state.set_llm_failed();
            }
        }
    });
}

/// Wait for a Ctrl-C (SIGINT) signal, then signal background tasks to stop
/// and return so `axum::serve` can drain in-flight connections gracefully.
async fn shutdown_signal(state: state::AppState) {
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to listen for ctrl_c signal");
    tracing::info!("Shutdown signal received — cancelling background tasks...");
    state.signal_shutdown();
    tracing::info!("Shutting down gracefully...");
}
