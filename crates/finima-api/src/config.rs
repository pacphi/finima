use serde::Deserialize;

/// Top-level application configuration deserialized from YAML files.
///
/// Loading order (later overrides earlier):
/// 1. `config/default.yaml`
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
}

#[derive(Debug, Deserialize, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AuthConfig {
    pub jwt_secret: String,
    pub magic_link_expiry_minutes: u64,
    pub from_email: String,
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

/// Validate critical configuration values at startup.
///
/// Panics if any of the following conditions are met:
/// - `database.url` is empty
/// - `auth.jwt_secret` equals the default placeholder in production
///
/// Logs a warning if the JWT secret is shorter than 32 characters.
pub fn validate_config(config: &AppConfig) {
    if config.database.url.is_empty() {
        panic!("FATAL: database.url must not be empty");
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
}

/// Load application configuration from YAML files and environment variables.
///
/// Reads `config/default.yaml`, then overlays `config/{APP_ENV}.yaml` (defaulting
/// to "development"), then overlays environment variables with prefix `APP` and
/// separator `__` (e.g., `APP__DATABASE__URL`).
pub fn load_config() -> Result<AppConfig, config::ConfigError> {
    let config = load_config_from("config")?;
    validate_config(&config);
    Ok(config)
}

/// Load configuration from a specific config directory path.
///
/// This is the same as `load_config()` but allows specifying where the
/// config files live. Useful for testing.
pub fn load_config_from(config_dir: &str) -> Result<AppConfig, config::ConfigError> {
    let app_env = std::env::var("APP_ENV").unwrap_or_else(|_| "development".to_string());

    let config = config::Config::builder()
        .add_source(config::File::with_name(&format!("{}/default", config_dir)))
        .add_source(config::File::with_name(&format!("{}/{}", config_dir, app_env)).required(false))
        .add_source(
            config::Environment::with_prefix("APP")
                .separator("__")
                .try_parsing(true),
        )
        .build()?;

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
  url: "postgres://test:test@localhost/test"
  max_connections: 5

auth:
  jwt_secret: "test-secret"
  magic_link_expiry_minutes: 10
  from_email: "test@test.com"
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
"#;
        let default_path = config_dir.join("default.yaml");
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
        assert_eq!(cfg.database.url, "postgres://test:test@localhost/test");
    }
}
