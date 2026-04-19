use async_trait::async_trait;
use crate::{EmbedError, EmbeddingProvider};

/// Candle-backed local embedder. **Phase 3 stub** — the feature gate
/// exists so `EMBEDDER=candle` builds compile and link, but the
/// provider returns `BackendUnavailable("candle: not yet wired")` at
/// runtime. Full implementation needs a sentence-transformer loader
/// distinct from the LLM chat loader; see ADR-017 Phase 3 follow-up.
pub struct CandleEmbedder {
    model_id: String,
    dim: usize,
}

impl CandleEmbedder {
    pub fn new(model_id: impl Into<String>, dim: usize) -> Self {
        Self { model_id: model_id.into(), dim }
    }
    pub fn model_id(&self) -> &str { &self.model_id }
}

#[async_trait]
impl EmbeddingProvider for CandleEmbedder {
    async fn embed(&self, _text: &str) -> Result<Vec<f32>, EmbedError> {
        tracing::warn!(
            model = %self.model_id,
            "CandleEmbedder is a Phase 3 stub; set EMBEDDER=ollama or wire the candle backend"
        );
        Err(EmbedError::BackendUnavailable("candle: not yet wired"))
    }
    fn dim(&self) -> usize { self.dim }
    fn backend(&self) -> &'static str { "candle" }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn candle_stub_errors() {
        let e = CandleEmbedder::new("sentence-transformers/all-MiniLM-L6-v2", 384);
        assert_eq!(e.dim(), 384);
        assert_eq!(e.backend(), "candle");
        assert!(e.embed("hello").await.is_err());
    }
}
