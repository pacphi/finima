use async_trait::async_trait;
use crate::{EmbedError, EmbeddingProvider};

/// No-op embedder used when `EMBEDDER=none`. Every `embed` call returns
/// `EmbedError::BackendUnavailable` so callers can skip the vector path
/// cleanly (rather than silently producing wrong results).
pub struct NoopEmbedder {
    dim: usize,
}

impl NoopEmbedder {
    pub fn new(dim: usize) -> Self { Self { dim } }
}

#[async_trait]
impl EmbeddingProvider for NoopEmbedder {
    async fn embed(&self, _text: &str) -> Result<Vec<f32>, EmbedError> {
        Err(EmbedError::BackendUnavailable("none"))
    }
    fn dim(&self) -> usize { self.dim }
    fn backend(&self) -> &'static str { "noop" }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn noop_always_errors() {
        let e = NoopEmbedder::new(384);
        assert_eq!(e.dim(), 384);
        assert_eq!(e.backend(), "noop");
        assert!(e.embed("anything").await.is_err());
    }
}
