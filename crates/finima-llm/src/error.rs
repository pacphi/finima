use thiserror::Error;

/// Errors that can occur in LLM operations.
#[derive(Debug, Error)]
pub enum LlmError {
    #[error("HTTP error: {0}")]
    Http(String),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Categorization failed: {0}")]
    Categorization(String),

    #[error("LLM timeout")]
    Timeout,

    #[error("Model loading failed: {0}")]
    ModelLoad(String),

    #[error("Inference error: {0}")]
    Inference(String),

    #[error("Hardware detection failed: {0}")]
    HardwareDetection(String),

    #[error("Unsupported configuration: {0}")]
    Configuration(String),
}
