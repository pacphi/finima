//! Embedding providers for Finima's semantic search (Tier 2, flow
//! matching). Matches the LLM flag family: `none` (noop), `ollama`
//! (HTTP), `candle` (local BERT sentence-transformer via candle-core).

use async_trait::async_trait;

#[cfg(feature = "ollama")]
pub mod ollama;
#[cfg(feature = "candle")]
pub mod candle;
pub mod noop;

#[cfg(feature = "ollama")]
pub use ollama::OllamaEmbedder;
#[cfg(feature = "candle")]
pub use candle::CandleEmbedder;
pub use noop::NoopEmbedder;

#[derive(Debug, thiserror::Error)]
pub enum EmbedError {
    #[error("embedding backend not available in this build: {0}")]
    BackendUnavailable(&'static str),
    #[error("HTTP error: {0}")]
    Http(String),
    #[error("response parse error: {0}")]
    Parse(String),
    #[error("dimension mismatch: expected {expected}, got {got}")]
    DimMismatch { expected: usize, got: usize },
    #[error("upstream timed out after {millis} ms")]
    Timeout { millis: u64 },
    #[error("other: {0}")]
    Other(String),
}

#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    /// Embed a single description. Output must be L2-normalized.
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbedError>;

    /// Optional batch hook. Default calls `embed` sequentially.
    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbedError> {
        let mut out = Vec::with_capacity(texts.len());
        for t in texts {
            out.push(self.embed(t).await?);
        }
        Ok(out)
    }

    /// Dimensionality of the emitted vectors. Used for sanity checks
    /// against `Tier2Config.dim`.
    fn dim(&self) -> usize;

    /// Short backend identifier for logging / metrics (e.g. "ollama",
    /// "candle", "noop").
    fn backend(&self) -> &'static str;
}

#[cfg(test)]
mod tests {
    // See per-module tests.
}
