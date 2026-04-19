use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::{EmbedError, EmbeddingProvider};

/// POST {base_url}/api/embeddings with body {"model":..., "prompt":...}
/// Response: {"embedding": [...]}.
pub struct OllamaEmbedder {
    base_url: String,
    model: String,
    dim: usize,
    client: reqwest::Client,
    timeout_millis: u64,
}

#[derive(Serialize)]
struct OllamaRequest<'a> {
    model: &'a str,
    prompt: &'a str,
}

#[derive(Deserialize)]
struct OllamaResponse {
    embedding: Vec<f32>,
}

impl OllamaEmbedder {
    pub fn new(base_url: impl Into<String>, model: impl Into<String>, dim: usize) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            model: model.into(),
            dim,
            client: reqwest::Client::new(),
            timeout_millis: 30_000,
        }
    }

    pub fn with_timeout_ms(mut self, ms: u64) -> Self {
        self.timeout_millis = ms;
        self
    }
}

#[async_trait]
impl EmbeddingProvider for OllamaEmbedder {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbedError> {
        let url = format!("{}/api/embeddings", self.base_url);
        let body = OllamaRequest {
            model: &self.model,
            prompt: text,
        };
        let resp = self
            .client
            .post(&url)
            .json(&body)
            .timeout(Duration::from_millis(self.timeout_millis))
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    EmbedError::Timeout {
                        millis: self.timeout_millis,
                    }
                } else {
                    EmbedError::Http(e.to_string())
                }
            })?;

        if !resp.status().is_success() {
            return Err(EmbedError::Http(format!("status {}", resp.status())));
        }

        let parsed: OllamaResponse = resp
            .json()
            .await
            .map_err(|e| EmbedError::Parse(e.to_string()))?;

        if parsed.embedding.len() != self.dim {
            return Err(EmbedError::DimMismatch {
                expected: self.dim,
                got: parsed.embedding.len(),
            });
        }

        let mut v = parsed.embedding;
        l2_normalize(&mut v);
        Ok(v)
    }

    fn dim(&self) -> usize {
        self.dim
    }
    fn backend(&self) -> &'static str {
        "ollama"
    }
}

fn l2_normalize(v: &mut [f32]) {
    let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-9);
    for x in v.iter_mut() {
        *x /= norm;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l2_normalize_unit() {
        let mut v = vec![3.0, 4.0, 0.0];
        l2_normalize(&mut v);
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-5);
    }

    #[test]
    fn construction_smoke() {
        let e = OllamaEmbedder::new("http://localhost:11434", "nomic-embed-text", 768)
            .with_timeout_ms(10_000);
        assert_eq!(e.dim(), 768);
        assert_eq!(e.backend(), "ollama");
    }
}
