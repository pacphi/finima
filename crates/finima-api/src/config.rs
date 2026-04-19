use std::collections::HashMap;

use finima_core::services::sign_normalizer::{
    SignConvention, SignConventions as CoreSignConventions,
};
use finima_core::AccountType;
use serde::Deserialize;
use uuid::Uuid;

/// Top-level application configuration deserialized from YAML files.
///
/// Loading order (later overrides earlier):
/// 1. Individual section files (`config/server.yaml`, `config/database.yaml`, etc.)
/// 2. `config/{APP_ENV}.yaml` (development | test | production)
/// 3. Environment variables prefixed with `APP` using `__` as separator
#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub auth: AuthConfig,
    pub resend: ResendConfig,
    pub llm: LlmConfig,
    pub feed: FeedConfig,
    pub logging: LoggingConfig,
    pub cors: CorsConfig,
    #[serde(default)]
    pub s3: S3Config,
    #[serde(default)]
    pub categories: Vec<CategoryEntry>,
    /// Maintainer-curated sign-convention rules used by the
    /// SignNormalizer at import time (see ADR-018).
    #[serde(default)]
    pub sign_conventions: SignConventionsConfig,
    /// Sankey visualization tuning (transfer-category exclusions, etc.).
    #[serde(default)]
    pub sankey: SankeyConfig,
    /// Recurring-payment detection thresholds (sliding window for variable
    /// classification, minimum occurrence count, etc.).
    #[serde(default)]
    pub recurring: RecurringConfig,
    /// Tiered categorization engine tuning (ADR-012). Includes the Tier 2
    /// backend selector + HNSW tuning when the `sona` feature is enabled
    /// in `finima-categorize`.
    #[serde(default)]
    pub categorize: CategorizeYamlConfig,
    /// Embedding provider selection (ADR-017 Phase 3.C). Used to populate
    /// vectors for Tier 2 and the flow-pattern matcher when the matching
    /// `embedder-*` Cargo feature is compiled in.
    #[serde(default)]
    pub embedder: EmbedderYamlConfig,
}

#[derive(Debug, Deserialize, Clone, serde::Serialize)]
pub struct SubcategoryEntry {
    pub key: String,
    pub label: String,
}

#[derive(Debug, Deserialize, Clone, serde::Serialize)]
pub struct CategoryEntry {
    pub key: String,
    pub label: String,
    #[serde(default)]
    pub subcategories: Vec<SubcategoryEntry>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DatabaseConfig {
    #[serde(default)]
    pub url: String,
    pub max_connections: u32,
    #[serde(default)]
    pub host: String,
    #[serde(default = "default_db_port")]
    pub port: u16,
    #[serde(default)]
    pub user: String,
    #[serde(default)]
    pub password: String,
    #[serde(default)]
    pub name: String,
}

fn default_db_port() -> u16 {
    5432
}

impl DatabaseConfig {
    /// Return the connection URL, constructing it from individual parts when
    /// `host` is set and `url` is empty.
    pub fn resolved_url(&self) -> String {
        if !self.url.is_empty() {
            return self.url.clone();
        }
        if !self.host.is_empty() {
            return format!(
                "postgres://{}:{}@{}:{}/{}",
                self.user, self.password, self.host, self.port, self.name
            );
        }
        String::new()
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct AuthConfig {
    pub jwt_secret: String,
    pub magic_link_expiry_minutes: u64,
    pub from_email: String,
    /// Public-facing base URL used in magic link emails (e.g. the frontend origin).
    pub public_url: String,
    /// Used by rate-limiting middleware at runtime; populated via serde.
    #[allow(dead_code)]
    pub rate_limit_per_hour: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ResendConfig {
    pub api_key: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LlmConfig {
    pub provider: String,
    /// Model identifier; read by LLM backend initialization.
    #[serde(default)]
    #[allow(dead_code)]
    pub model: String,
    /// Candle backend settings; read when the `candle` feature is enabled.
    #[serde(default)]
    #[allow(dead_code)]
    pub candle: CandleConfig,
    /// Ollama backend settings; read when the `ollama` feature is enabled.
    #[serde(default)]
    #[allow(dead_code)]
    pub ollama: OllamaConfig,
    /// Populated via serde for use by the categorization pipeline.
    #[serde(default = "default_batch_size")]
    #[allow(dead_code)]
    pub batch_size: usize,
    /// Populated via serde for use by the categorization pipeline.
    #[serde(default = "default_confidence_threshold")]
    #[allow(dead_code)]
    pub confidence_threshold: f64,
    /// Populated via serde for use by the LLM client.
    #[serde(default = "default_timeout_seconds")]
    #[allow(dead_code)]
    pub timeout_seconds: u64,
    /// Populated via serde for use by the LLM client.
    #[serde(default = "default_max_retries")]
    #[allow(dead_code)]
    pub max_retries: u32,
    /// Number of parallel batch requests to send to the LLM.
    #[serde(default = "default_parallel_requests")]
    #[allow(dead_code)]
    pub parallel_requests: usize,
    /// Context window size in tokens for the LLM.
    #[serde(default = "default_num_ctx")]
    #[allow(dead_code)]
    pub num_ctx: usize,
}

fn default_batch_size() -> usize {
    20
}
fn default_confidence_threshold() -> f64 {
    0.7
}
fn default_timeout_seconds() -> u64 {
    60
}
fn default_max_retries() -> u32 {
    2
}
fn default_parallel_requests() -> usize {
    1
}
fn default_num_ctx() -> usize {
    4096
}

/// Candle backend configuration; fields are read when the `candle` feature is enabled.
#[derive(Debug, Deserialize, Clone)]
pub struct CandleConfig {
    #[serde(default = "default_candle_model_id")]
    #[allow(dead_code)]
    pub model_id: String,
    #[serde(default)]
    #[allow(dead_code)]
    pub model_path: String,
    #[serde(default = "default_quantization")]
    #[allow(dead_code)]
    pub quantization: String,
    #[serde(default = "default_device")]
    #[allow(dead_code)]
    pub device: String,
    #[serde(default = "default_context_length")]
    #[allow(dead_code)]
    pub context_length: usize,
    #[serde(default)]
    #[allow(dead_code)]
    pub threads: usize,
}

fn default_candle_model_id() -> String {
    "google/gemma-4-E4B-it".to_string()
}
fn default_quantization() -> String {
    "Q4_K_M".to_string()
}
fn default_device() -> String {
    "auto".to_string()
}
fn default_context_length() -> usize {
    8192
}

impl Default for CandleConfig {
    fn default() -> Self {
        Self {
            model_id: default_candle_model_id(),
            model_path: String::new(),
            quantization: default_quantization(),
            device: default_device(),
            context_length: default_context_length(),
            threads: 0,
        }
    }
}

/// Ollama backend configuration; fields are read when the `ollama` feature is enabled.
#[derive(Debug, Deserialize, Clone)]
pub struct OllamaConfig {
    #[serde(default = "default_ollama_url")]
    #[allow(dead_code)]
    pub url: String,
    #[serde(default = "default_ollama_model")]
    #[allow(dead_code)]
    pub model: String,
}

fn default_ollama_url() -> String {
    "http://localhost:11434".to_string()
}
fn default_ollama_model() -> String {
    "gemma4:26b-a4b-it-q4_K_M".to_string()
}

impl Default for OllamaConfig {
    fn default() -> Self {
        Self {
            url: default_ollama_url(),
            model: default_ollama_model(),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct FeedSource {
    pub name: String,
    pub url: String,
    pub topic: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct FeedConfig {
    /// Populated via serde; used by the feed polling scheduler.
    #[allow(dead_code)]
    pub poll_interval_hours: u32,
    pub sources: Vec<FeedSource>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LoggingConfig {
    pub level: String,
    pub format: String,
    /// Directory for rolling log files. When set, a daily-rotating file
    /// appender writes plain-text logs alongside the console output.
    #[serde(default)]
    pub log_dir: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CorsConfig {
    pub allowed_origins: Vec<String>,
}

/// Configuration for S3-compatible object storage (MinIO, AWS S3, etc.).
#[derive(Debug, Deserialize, Clone)]
pub struct S3Config {
    /// Endpoint URL, e.g. "http://minio:9000" or "https://s3.amazonaws.com".
    pub endpoint_url: String,
    /// AWS region, e.g. "us-east-1".
    pub region: String,
    /// Bucket name for uploaded files.
    pub bucket: String,
    /// Access key ID (maps to AWS_ACCESS_KEY_ID for AWS).
    pub access_key_id: String,
    /// Secret access key (maps to AWS_SECRET_ACCESS_KEY for AWS).
    pub secret_access_key: String,
    /// Use path-style addressing. Must be `true` for MinIO; `false` for AWS.
    pub force_path_style: bool,
}

impl Default for S3Config {
    fn default() -> Self {
        Self {
            endpoint_url: "http://localhost:9000".to_string(),
            region: "us-east-1".to_string(),
            bucket: "finima-uploads".to_string(),
            access_key_id: "minioadmin".to_string(),
            secret_access_key: "minioadmin".to_string(),
            force_path_style: true,
        }
    }
}

// ───────────────────────────────────────────────────────────────────
// Sign conventions (ADR-018)
// ───────────────────────────────────────────────────────────────────

/// Wire-format enum for `sign_convention_*` values in YAML/JSON.
/// Maps 1:1 to [`finima_core::services::sign_normalizer::SignConvention`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignConventionConfig {
    /// Positive amount = inflow (money in), negative = outflow.
    PositiveMeansInflow,
    /// Positive amount = outflow (money out), negative = inflow.
    PositiveMeansOutflow,
}

impl From<SignConventionConfig> for SignConvention {
    fn from(c: SignConventionConfig) -> Self {
        match c {
            SignConventionConfig::PositiveMeansInflow => SignConvention::PositiveMeansInflow,
            SignConventionConfig::PositiveMeansOutflow => SignConvention::PositiveMeansOutflow,
        }
    }
}

/// YAML-shipped sign-convention registry. Maintainers add entries here
/// to capture per-institution quirks (e.g. Chase exporting credit-card
/// charges as negative). End users do NOT edit this — they correct
/// individual accounts via the UI, which writes a per-account override
/// stored on the `accounts` table.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct SignConventionsConfig {
    /// Optional per-account pins. Rare; primarily for one-off
    /// migrations or test scenarios. The UI normally writes per-account
    /// overrides directly to the database.
    #[serde(default)]
    pub by_account_id: HashMap<Uuid, SignConventionConfig>,
    /// Maintainer-curated per-institution rules. Keys are matched
    /// case-insensitively against the account's institution name.
    #[serde(default)]
    pub by_institution: HashMap<String, SignConventionConfig>,
}

impl SignConventionsConfig {
    /// Convert this YAML-friendly config into the core service type
    /// used by `SignNormalizer`. Built-in defaults are merged in by
    /// `SignNormalizer::new`, so callers don't need to pre-populate
    /// `defaults_by_account_type`.
    pub fn into_service_rules(self) -> CoreSignConventions {
        let mut rules = CoreSignConventions::default();
        for (id, c) in self.by_account_id {
            rules.by_account_id.insert(id, c.into());
        }
        for (name, c) in self.by_institution {
            rules.by_institution.insert(name.to_lowercase(), c.into());
        }
        rules
    }
}

// ───────────────────────────────────────────────────────────────────
// Sankey configuration (ADR-008 Amendment 2)
// ───────────────────────────────────────────────────────────────────

/// Sankey aggregation/rendering tuning.
#[derive(Debug, Clone, Deserialize)]
pub struct SankeyConfig {
    /// Categories whose transactions are NOT counted as "spending"
    /// in the Sankey (because they represent transfers, not consumption).
    /// Applied uniformly to all account types. Default:
    /// `["transfer", "debt_payment"]`.
    #[serde(default = "default_transfer_categories")]
    pub transfer_categories: Vec<String>,
}

fn default_transfer_categories() -> Vec<String> {
    vec!["transfer".to_string(), "debt_payment".to_string()]
}

impl Default for SankeyConfig {
    fn default() -> Self {
        Self {
            transfer_categories: default_transfer_categories(),
        }
    }
}

// ───────────────────────────────────────────────────────────────────
// Recurring detection (configurable thresholds)
// ───────────────────────────────────────────────────────────────────

/// Maintainer-tunable thresholds for the recurring-transaction detector.
///
/// `min_occurrences_for_variable` is the minimum number of times a candidate
/// merchant must occur within the sliding window to be kept when its inter-
/// date intervals don't fit any fixed cadence. `variable_window_months`
/// controls the size of that window (anchored on the candidate's most recent
/// transaction).
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct RecurringConfig {
    #[serde(default = "default_recurring_min_occurrences_for_variable")]
    pub min_occurrences_for_variable: usize,
    #[serde(default = "default_recurring_variable_window_months")]
    pub variable_window_months: u32,
    /// Minimum observations required to promote a sub-annual fixed cadence
    /// (Weekly / Biweekly / Monthly / Quarterly). See ADR-019.
    #[serde(default = "default_recurring_min_occurrences_for_fixed")]
    pub min_occurrences_for_fixed: usize,
}

fn default_recurring_min_occurrences_for_variable() -> usize {
    finima_analysis::RecurringDetectorConfig::DEFAULT_MIN_OCCURRENCES_FOR_VARIABLE
}

fn default_recurring_variable_window_months() -> u32 {
    finima_analysis::RecurringDetectorConfig::DEFAULT_VARIABLE_WINDOW_MONTHS
}

fn default_recurring_min_occurrences_for_fixed() -> usize {
    finima_analysis::RecurringDetectorConfig::DEFAULT_MIN_OCCURRENCES_FOR_FIXED
}

impl Default for RecurringConfig {
    fn default() -> Self {
        Self {
            min_occurrences_for_variable: default_recurring_min_occurrences_for_variable(),
            variable_window_months: default_recurring_variable_window_months(),
            min_occurrences_for_fixed: default_recurring_min_occurrences_for_fixed(),
        }
    }
}

// ───────────────────────────────────────────────────────────────────
// Categorization engine (ADR-012) — YAML-backed tunables
// ───────────────────────────────────────────────────────────────────

/// YAML mirror of [`finima_categorize::CategorizeConfig`]. Kept separate so
/// the crate-level struct stays free of serde/config dependencies.
#[derive(Debug, Clone, Deserialize)]
pub struct CategorizeYamlConfig {
    #[serde(default = "default_fuzzy_threshold")]
    pub fuzzy_threshold: f64,
    #[serde(default = "default_pattern_min_confidence")]
    pub pattern_min_confidence: f64,
    #[serde(default = "default_semantic_min_confidence")]
    pub semantic_min_confidence: f64,
    #[serde(default = "default_prefix_length")]
    pub prefix_length: usize,
    #[serde(default)]
    pub tier2: Tier2YamlConfig,
}

fn default_fuzzy_threshold() -> f64 {
    0.88
}
fn default_pattern_min_confidence() -> f64 {
    0.70
}
fn default_semantic_min_confidence() -> f64 {
    0.85
}
fn default_prefix_length() -> usize {
    3
}

impl Default for CategorizeYamlConfig {
    fn default() -> Self {
        Self {
            fuzzy_threshold: default_fuzzy_threshold(),
            pattern_min_confidence: default_pattern_min_confidence(),
            semantic_min_confidence: default_semantic_min_confidence(),
            prefix_length: default_prefix_length(),
            tier2: Tier2YamlConfig::default(),
        }
    }
}

/// YAML mirror of [`finima_categorize::config::Tier2Config`].
#[derive(Debug, Clone, Deserialize)]
pub struct Tier2YamlConfig {
    #[serde(default = "default_tier2_backend")]
    pub backend: String,
    #[serde(default = "default_tier2_dim")]
    pub dim: usize,
    #[serde(default = "default_tier2_hnsw_m")]
    pub hnsw_m: usize,
    #[serde(default = "default_tier2_hnsw_ef_construction")]
    pub hnsw_ef_construction: usize,
    #[serde(default = "default_tier2_hnsw_ef_search")]
    pub hnsw_ef_search: usize,
    #[serde(default = "default_tier2_bootstrap_on_start")]
    pub bootstrap_on_start: bool,
    #[serde(default)]
    pub bootstrap_max_examples: usize,
}

fn default_tier2_backend() -> String {
    "jaccard".to_string()
}
fn default_tier2_dim() -> usize {
    384
}
fn default_tier2_hnsw_m() -> usize {
    32
}
fn default_tier2_hnsw_ef_construction() -> usize {
    200
}
fn default_tier2_hnsw_ef_search() -> usize {
    100
}
fn default_tier2_bootstrap_on_start() -> bool {
    true
}

impl Default for Tier2YamlConfig {
    fn default() -> Self {
        Self {
            backend: default_tier2_backend(),
            dim: default_tier2_dim(),
            hnsw_m: default_tier2_hnsw_m(),
            hnsw_ef_construction: default_tier2_hnsw_ef_construction(),
            hnsw_ef_search: default_tier2_hnsw_ef_search(),
            bootstrap_on_start: default_tier2_bootstrap_on_start(),
            bootstrap_max_examples: 0,
        }
    }
}

impl From<CategorizeYamlConfig> for finima_categorize::CategorizeConfig {
    fn from(c: CategorizeYamlConfig) -> Self {
        finima_categorize::CategorizeConfig {
            fuzzy_threshold: c.fuzzy_threshold,
            pattern_min_confidence: c.pattern_min_confidence,
            semantic_min_confidence: c.semantic_min_confidence,
            prefix_length: c.prefix_length,
            tier2: c.tier2.into(),
        }
    }
}

impl From<Tier2YamlConfig> for finima_categorize::config::Tier2Config {
    fn from(c: Tier2YamlConfig) -> Self {
        let backend = finima_categorize::config::Tier2Backend::parse(&c.backend)
            .unwrap_or_else(|| {
                tracing::warn!(
                    value = %c.backend,
                    "categorize.tier2.backend is not a recognized value; falling back to jaccard"
                );
                finima_categorize::config::Tier2Backend::Jaccard
            });
        finima_categorize::config::Tier2Config {
            backend,
            dim: c.dim,
            hnsw_m: c.hnsw_m,
            hnsw_ef_construction: c.hnsw_ef_construction,
            hnsw_ef_search: c.hnsw_ef_search,
            bootstrap_on_start: c.bootstrap_on_start,
            bootstrap_max_examples: c.bootstrap_max_examples,
        }
    }
}

impl From<RecurringConfig> for finima_analysis::RecurringDetectorConfig {
    fn from(c: RecurringConfig) -> Self {
        finima_analysis::RecurringDetectorConfig {
            min_occurrences_for_variable: c.min_occurrences_for_variable,
            variable_window_months: c.variable_window_months,
            min_occurrences_for_fixed: c.min_occurrences_for_fixed,
        }
    }
}

// ───────────────────────────────────────────────────────────────────
// Embedder (ADR-017 Phase 3.C)
// ───────────────────────────────────────────────────────────────────

/// YAML mirror of the embedding-provider configuration. `backend` selects
/// which provider is instantiated at startup; if the matching Cargo feature
/// is not compiled in, `AppState::new` falls back to `NoopEmbedder` with a
/// warning.
#[derive(Debug, Clone, Deserialize)]
pub struct EmbedderYamlConfig {
    #[serde(default = "default_embedder_backend")]
    pub backend: String,
    #[serde(default = "default_embedder_dim")]
    pub dim: usize,
    /// Ollama sub-config; read only when `embedder-ollama` is enabled.
    #[serde(default)]
    #[allow(dead_code)]
    pub ollama: EmbedderOllamaConfig,
    /// Candle sub-config; read only when `embedder-candle` is enabled.
    #[serde(default)]
    #[allow(dead_code)]
    pub candle: EmbedderCandleConfig,
}

fn default_embedder_backend() -> String {
    "none".to_string()
}
fn default_embedder_dim() -> usize {
    384
}

impl Default for EmbedderYamlConfig {
    fn default() -> Self {
        Self {
            backend: default_embedder_backend(),
            dim: default_embedder_dim(),
            ollama: EmbedderOllamaConfig::default(),
            candle: EmbedderCandleConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct EmbedderOllamaConfig {
    #[serde(default = "default_embedder_ollama_url")]
    #[allow(dead_code)]
    pub url: String,
    #[serde(default = "default_embedder_ollama_model")]
    #[allow(dead_code)]
    pub model: String,
    #[serde(default = "default_embedder_ollama_timeout_millis")]
    #[allow(dead_code)]
    pub timeout_millis: u64,
}

fn default_embedder_ollama_url() -> String {
    "http://localhost:11434".to_string()
}
fn default_embedder_ollama_model() -> String {
    "nomic-embed-text".to_string()
}
fn default_embedder_ollama_timeout_millis() -> u64 {
    30_000
}

impl Default for EmbedderOllamaConfig {
    fn default() -> Self {
        Self {
            url: default_embedder_ollama_url(),
            model: default_embedder_ollama_model(),
            timeout_millis: default_embedder_ollama_timeout_millis(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct EmbedderCandleConfig {
    #[serde(default = "default_embedder_candle_model_id")]
    #[allow(dead_code)]
    pub model_id: String,
}

fn default_embedder_candle_model_id() -> String {
    "sentence-transformers/all-MiniLM-L6-v2".to_string()
}

impl Default for EmbedderCandleConfig {
    fn default() -> Self {
        Self {
            model_id: default_embedder_candle_model_id(),
        }
    }
}

#[allow(dead_code)] // referenced via AccountType import only when wired into ingest
fn _account_type_marker(_t: AccountType) {}

/// Validate critical configuration values at startup.
///
/// Panics if any of the following conditions are met:
/// - `database.url` is empty and cannot be constructed from parts
/// - `auth.jwt_secret` equals the default placeholder in production
///
/// Logs a warning if the JWT secret is shorter than 32 characters.
pub fn validate_config(config: &AppConfig) {
    if config.database.resolved_url().is_empty() {
        panic!("FATAL: database.url must not be empty (set url directly or provide host/user/password/name)");
    }

    let app_env = std::env::var("APP_ENV").unwrap_or_else(|_| "development".to_string());

    if config.auth.jwt_secret == "change-me-in-production" && app_env == "production" {
        panic!("FATAL: JWT secret must be changed from default value in production");
    }

    if config.auth.jwt_secret.len() < 32 {
        eprintln!(
            "WARNING: auth.jwt_secret is only {} bytes; consider using at least 32 bytes for adequate security",
            config.auth.jwt_secret.len()
        );
    }

    // Log the resolved Tier 2 backend so operators can verify that the
    // `sona` feature (or its absence) matches the YAML selection. This
    // also keeps the `categorize` field reachable at the type level.
    let crate_cfg: finima_categorize::CategorizeConfig = config.categorize.clone().into();
    let resolved = crate_cfg.tier2.resolved_backend();
    tracing::info!(
        requested = %crate_cfg.tier2.backend.as_str(),
        resolved = %resolved.as_str(),
        dim = crate_cfg.tier2.dim,
        "Tier 2 categorization backend"
    );

    // Log the resolved embedder backend + dim so operators can verify
    // the EMBEDDER= Cargo feature matches the YAML selection. Actual
    // construction happens in `AppState::new`, which may downgrade to
    // `noop` if the requested backend's feature is disabled.
    tracing::info!(
        backend = %config.embedder.backend,
        dim = config.embedder.dim,
        "Embedder backend (YAML)"
    );
}

/// Load application configuration from YAML files and environment variables.
///
/// Loads individual section files (`config/server.yaml`, `config/database.yaml`,
/// etc.), then `config/{APP_ENV}.yaml` (defaulting to "development"), then
/// environment variables with prefix `APP` and separator `__` (e.g.,
/// `APP__DATABASE__PASSWORD`).
pub fn load_config() -> Result<AppConfig, config::ConfigError> {
    let config = load_config_from("config")?;
    validate_config(&config);
    Ok(config)
}

/// Load configuration from a specific config directory path.
///
/// This is the same as `load_config()` but allows specifying where the
/// config files live. Useful for testing.
///
/// Loading order (later overrides earlier):
/// 1. Individual section files (`server.yaml`, `database.yaml`, etc.)
/// 2. `{APP_ENV}.yaml` — environment-specific overlay
/// 3. Environment variables prefixed with `APP` using `__` as separator
pub fn load_config_from(config_dir: &str) -> Result<AppConfig, config::ConfigError> {
    let app_env = std::env::var("APP_ENV").unwrap_or_else(|_| "development".to_string());

    // Individual section files are loaded first.  Each file is optional so
    // the loader is flexible about which files are present.
    let section_files = [
        "server",
        "database",
        "auth",
        "llm",
        "storage",
        "categories",
        "services",
        "logging",
        "sankey",
        "recurring",
        "categorize",
        "embedder",
    ];

    let mut builder = config::Config::builder();

    for section in &section_files {
        builder = builder.add_source(
            config::File::with_name(&format!("{}/{}", config_dir, section)).required(false),
        );
    }

    // Environment overlay (development / test / production).
    builder = builder.add_source(
        config::File::with_name(&format!("{}/{}", config_dir, app_env)).required(false),
    );

    // Environment variables take the highest precedence.
    builder = builder.add_source(
        config::Environment::with_prefix("APP")
            .separator("__")
            .try_parsing(true),
    );

    let config = builder.build()?;
    config.try_deserialize()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_test_yaml(dir: &std::path::Path) -> String {
        let config_dir = dir.join("cfg");
        std::fs::create_dir_all(&config_dir).unwrap();

        let yaml = r#"
server:
  host: "127.0.0.1"
  port: 4000

database:
  host: "localhost"
  port: 5432
  user: "test"
  password: "test"
  name: "test"
  max_connections: 5

auth:
  jwt_secret: "test-secret"
  magic_link_expiry_minutes: 10
  from_email: "test@test.com"
  public_url: "http://localhost:5173"
  rate_limit_per_hour: 3

resend:
  api_key: ""

llm:
  provider: "candle"
  model: "auto"
  candle:
    model_id: "google/gemma-4-E4B-it"
    model_path: ""
    quantization: "Q4_K_M"
    device: "auto"
    context_length: 8192
    threads: 0
  ollama:
    url: "http://localhost:11434"
    model: "test-model"
  batch_size: 20
  confidence_threshold: 0.7
  timeout_seconds: 60
  max_retries: 2

feed:
  poll_interval_hours: 12
  sources: []

logging:
  level: "info"
  format: "pretty"
  log_dir: ""

cors:
  allowed_origins:
    - "http://localhost:3000"

s3:
  endpoint_url: "http://localhost:9000"
  region: "us-east-1"
  bucket: "finima-uploads-test"
  access_key_id: "minioadmin"
  secret_access_key: "minioadmin"
  force_path_style: true

categories:
  - key: housing
    label: Housing
    subcategories:
      - key: rent
        label: Rent
      - key: mortgage
        label: Mortgage
  - key: other
    label: Other
"#;
        // Write as server.yaml — the loader reads individual section files,
        // and the config crate merges all top-level keys from any source.
        let default_path = config_dir.join("server.yaml");
        let mut file = std::fs::File::create(&default_path).unwrap();
        file.write_all(yaml.as_bytes()).unwrap();

        config_dir.to_string_lossy().to_string()
    }

    #[test]
    fn load_config_from_yaml() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = write_test_yaml(dir.path());

        let cfg = load_config_from(&config_path).expect("should load config");
        assert_eq!(cfg.server.host, "127.0.0.1");
        assert_eq!(cfg.server.port, 4000);
        assert_eq!(cfg.database.max_connections, 5);
        assert_eq!(cfg.auth.magic_link_expiry_minutes, 10);
        assert_eq!(cfg.llm.provider, "candle");
        assert_eq!(cfg.llm.model, "auto");
        assert_eq!(cfg.llm.candle.model_id, "google/gemma-4-E4B-it");
        assert_eq!(cfg.feed.poll_interval_hours, 12);
        assert!(cfg.feed.sources.is_empty());
        assert_eq!(cfg.logging.level, "info");
        assert_eq!(cfg.cors.allowed_origins.len(), 1);
    }

    #[test]
    fn sign_conventions_default_is_empty() {
        let cfg = SignConventionsConfig::default();
        assert!(cfg.by_account_id.is_empty());
        assert!(cfg.by_institution.is_empty());
    }

    #[test]
    fn sign_conventions_parse_from_yaml() {
        let yaml = r#"
by_institution:
  chase: positive_means_inflow
  citi:  positive_means_inflow
by_account_id:
  "00000000-0000-0000-0000-000000000001": positive_means_outflow
"#;
        let cfg: SignConventionsConfig = serde_yaml::from_str(yaml).expect("parse");
        assert_eq!(
            cfg.by_institution.get("chase"),
            Some(&SignConventionConfig::PositiveMeansInflow)
        );
        assert_eq!(
            cfg.by_institution.get("citi"),
            Some(&SignConventionConfig::PositiveMeansInflow)
        );
        let pinned = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();
        assert_eq!(
            cfg.by_account_id.get(&pinned),
            Some(&SignConventionConfig::PositiveMeansOutflow)
        );
    }

    #[test]
    fn sign_conventions_into_service_rules_lowercases_institutions() {
        let mut cfg = SignConventionsConfig::default();
        cfg.by_institution.insert(
            "CHASE".to_string(),
            SignConventionConfig::PositiveMeansInflow,
        );
        let rules = cfg.into_service_rules();
        assert!(rules.by_institution.contains_key("chase"));
        assert!(!rules.by_institution.contains_key("CHASE"));
    }

    #[test]
    fn sankey_config_default_excludes_transfer_categories() {
        let cfg = SankeyConfig::default();
        assert!(cfg.transfer_categories.contains(&"transfer".to_string()));
        assert!(cfg
            .transfer_categories
            .contains(&"debt_payment".to_string()));
    }

    #[test]
    fn sankey_config_parses_from_yaml() {
        let yaml = r#"transfer_categories: ["transfer", "debt_payment", "investment_buy"]"#;
        let cfg: SankeyConfig = serde_yaml::from_str(yaml).expect("parse");
        assert_eq!(cfg.transfer_categories.len(), 3);
        assert!(cfg
            .transfer_categories
            .contains(&"investment_buy".to_string()));
    }

    #[test]
    fn load_config_deserializes_all_sections() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = write_test_yaml(dir.path());

        let cfg = load_config_from(&config_path).expect("should load config");
        assert_eq!(cfg.auth.from_email, "test@test.com");
        assert_eq!(cfg.auth.rate_limit_per_hour, 3);
        assert!(cfg.resend.api_key.is_empty());
        assert_eq!(cfg.llm.ollama.url, "http://localhost:11434");
        assert_eq!(cfg.llm.ollama.model, "test-model");
        assert_eq!(cfg.llm.candle.quantization, "Q4_K_M");
        assert_eq!(cfg.llm.candle.context_length, 8192);
        assert_eq!(cfg.llm.batch_size, 20);
        assert_eq!(cfg.llm.confidence_threshold, 0.7);
        assert_eq!(cfg.llm.timeout_seconds, 60);
        assert_eq!(cfg.llm.max_retries, 2);
        assert_eq!(cfg.logging.format, "pretty");
        assert_eq!(
            cfg.database.resolved_url(),
            "postgres://test:test@localhost:5432/test"
        );
    }
}
