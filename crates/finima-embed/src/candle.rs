//! Real Candle sentence-transformer embedder.
//!
//! Default model: `sentence-transformers/all-MiniLM-L6-v2` (384-dim,
//! BERT-family). Model weights, config, and tokenizer are downloaded
//! once via `hf-hub`; the `BertModel` + `Tokenizer` are then held in
//! memory for the life of the embedder.
//!
//! Pipeline per [`EmbeddingProvider::embed`]:
//!   1. tokenize -> `input_ids`, `attention_mask`
//!   2. BERT forward pass on the selected `candle_core::Device`
//!   3. attention-mask-weighted mean pooling over the sequence axis
//!   4. L2 normalization -> unit vector of length `hidden_size`

use async_trait::async_trait;
use std::path::PathBuf;
use std::sync::Arc;

use candle_core::{DType, Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config as BertConfig, DTYPE};
use hf_hub::api::sync::Api;
use tokenizers::Tokenizer;
use tokio::task;

use crate::{EmbedError, EmbeddingProvider};

/// Local sentence-transformer embedder backed by Candle.
pub struct CandleEmbedder {
    model: Arc<BertModel>,
    tokenizer: Arc<Tokenizer>,
    device: Device,
    dim: usize,
    model_id: String,
}

/// Explicit device selection. Use [`CandleDevice::Auto`] to pick the
/// best accelerator available at build time (CUDA > Metal > CPU).
#[derive(Debug, Clone, Copy)]
pub enum CandleDevice {
    Cpu,
    Metal,
    Cuda,
    Auto,
}

impl CandleEmbedder {
    /// Load the model using [`CandleDevice::Auto`].
    pub fn load(model_id: impl Into<String>, dim: usize) -> Result<Self, EmbedError> {
        Self::load_on(model_id, dim, CandleDevice::Auto)
    }

    /// Load the model on a specific device. If the requested device's
    /// feature flag is not active in this build, falls back to CPU.
    pub fn load_on(
        model_id: impl Into<String>,
        dim: usize,
        device: CandleDevice,
    ) -> Result<Self, EmbedError> {
        let model_id = model_id.into();
        let device =
            resolve_device(device).map_err(|e| EmbedError::Other(format!("device init: {e}")))?;

        let api = Api::new().map_err(|e| EmbedError::Other(format!("hf-hub api: {e}")))?;
        let repo = api.model(model_id.clone());
        let config_file: PathBuf = repo
            .get("config.json")
            .map_err(|e| EmbedError::Other(format!("download config.json: {e}")))?;
        let tokenizer_file: PathBuf = repo
            .get("tokenizer.json")
            .map_err(|e| EmbedError::Other(format!("download tokenizer.json: {e}")))?;
        let weights_file: PathBuf = repo
            .get("model.safetensors")
            .or_else(|_| repo.get("pytorch_model.bin"))
            .map_err(|e| EmbedError::Other(format!("download weights: {e}")))?;

        let cfg_json = std::fs::read_to_string(&config_file)
            .map_err(|e| EmbedError::Other(format!("read config.json: {e}")))?;
        let bert_cfg: BertConfig = serde_json::from_str(&cfg_json)
            .map_err(|e| EmbedError::Parse(format!("parse config.json: {e}")))?;

        let is_safetensors =
            weights_file.extension().and_then(|s| s.to_str()) == Some("safetensors");
        let vb = if is_safetensors {
            // SAFETY: file is read-only, mmap lifetime is tied to the
            // resulting VarBuilder which we immediately consume.
            unsafe {
                VarBuilder::from_mmaped_safetensors(
                    std::slice::from_ref(&weights_file),
                    DTYPE,
                    &device,
                )
            }
            .map_err(|e| EmbedError::Other(format!("safetensors load: {e}")))?
        } else {
            VarBuilder::from_pth(&weights_file, DTYPE, &device)
                .map_err(|e| EmbedError::Other(format!("pytorch weights load: {e}")))?
        };

        let model = BertModel::load(vb, &bert_cfg)
            .map_err(|e| EmbedError::Other(format!("BertModel::load: {e}")))?;
        let tokenizer = Tokenizer::from_file(tokenizer_file)
            .map_err(|e| EmbedError::Other(format!("tokenizer load: {e}")))?;

        tracing::info!(
            model = %model_id,
            dim = dim,
            device = device_kind(&device),
            "loaded Candle sentence-transformer"
        );

        Ok(Self {
            model: Arc::new(model),
            tokenizer: Arc::new(tokenizer),
            device,
            dim,
            model_id,
        })
    }

    pub fn model_id(&self) -> &str {
        &self.model_id
    }

    /// Synchronous forward pass. Runs tokenization, BERT forward,
    /// masked mean-pool, and L2 normalization. Returned vector has
    /// length `self.dim` and unit L2 norm.
    fn embed_blocking(&self, text: &str) -> Result<Vec<f32>, EmbedError> {
        let enc = self
            .tokenizer
            .encode(text, true)
            .map_err(|e| EmbedError::Parse(format!("tokenize: {e}")))?;
        let ids: Vec<u32> = enc.get_ids().to_vec();
        let mask: Vec<u32> = enc.get_attention_mask().to_vec();
        let n = ids.len();
        if n == 0 {
            return Err(EmbedError::Parse("tokenizer produced 0 tokens".into()));
        }

        let input_ids = Tensor::new(ids.as_slice(), &self.device)
            .and_then(|t| t.reshape((1, n)))
            .map_err(|e| EmbedError::Other(format!("input_ids: {e}")))?;
        let attn_u32 = Tensor::new(mask.as_slice(), &self.device)
            .and_then(|t| t.reshape((1, n)))
            .map_err(|e| EmbedError::Other(format!("attn_mask: {e}")))?;
        let token_type_ids = input_ids
            .zeros_like()
            .map_err(|e| EmbedError::Other(format!("token_type_ids: {e}")))?;

        let hidden = self
            .model
            .forward(&input_ids, &token_type_ids, Some(&attn_u32))
            .map_err(|e| EmbedError::Other(format!("bert forward: {e}")))?;
        // hidden shape: [1, n, hidden_dim]

        // Mean-pool with attention mask.
        let attn_f = attn_u32
            .to_dtype(DType::F32)
            .map_err(|e| EmbedError::Other(format!("attn f32: {e}")))?;
        let mask_expanded = attn_f
            .unsqueeze(2)
            .map_err(|e| EmbedError::Other(format!("mask unsqueeze: {e}")))?;
        let masked = hidden
            .broadcast_mul(&mask_expanded)
            .map_err(|e| EmbedError::Other(format!("masked hidden: {e}")))?;
        let summed = masked
            .sum(1)
            .map_err(|e| EmbedError::Other(format!("sum seq: {e}")))?;
        // Token counts per row, guarded against all-pad inputs.
        let counts = attn_f
            .sum(1)
            .and_then(|t| t.clamp(1e-9_f64, f64::INFINITY))
            .map_err(|e| EmbedError::Other(format!("counts: {e}")))?;
        let counts = counts
            .unsqueeze(1)
            .map_err(|e| EmbedError::Other(format!("counts unsqueeze: {e}")))?;
        let pooled = summed
            .broadcast_div(&counts)
            .map_err(|e| EmbedError::Other(format!("mean-pool div: {e}")))?;
        // pooled shape: [1, hidden_dim]

        // L2 normalize along the feature axis.
        let norm = pooled
            .sqr()
            .and_then(|t| t.sum_keepdim(1))
            .and_then(|t| t.sqrt())
            .map_err(|e| EmbedError::Other(format!("l2 norm: {e}")))?;
        let unit = pooled
            .broadcast_div(&norm)
            .and_then(|t| t.squeeze(0))
            .map_err(|e| EmbedError::Other(format!("unit vec: {e}")))?;

        let vec = unit
            .to_vec1::<f32>()
            .map_err(|e| EmbedError::Other(format!("to_vec1: {e}")))?;

        if vec.len() != self.dim {
            return Err(EmbedError::DimMismatch {
                expected: self.dim,
                got: vec.len(),
            });
        }
        Ok(vec)
    }
}

fn resolve_device(req: CandleDevice) -> Result<Device, candle_core::Error> {
    match req {
        CandleDevice::Cpu => Ok(Device::Cpu),
        #[cfg(feature = "candle-metal")]
        CandleDevice::Metal => Device::new_metal(0),
        #[cfg(feature = "candle-cuda")]
        CandleDevice::Cuda => Device::new_cuda(0),
        #[cfg(not(feature = "candle-metal"))]
        CandleDevice::Metal => Ok(Device::Cpu),
        #[cfg(not(feature = "candle-cuda"))]
        CandleDevice::Cuda => Ok(Device::Cpu),
        CandleDevice::Auto => {
            #[cfg(feature = "candle-cuda")]
            {
                return Device::new_cuda(0);
            }
            #[cfg(feature = "candle-metal")]
            {
                return Device::new_metal(0);
            }
            #[allow(unreachable_code)]
            {
                Ok(Device::Cpu)
            }
        }
    }
}

fn device_kind(d: &Device) -> &'static str {
    if d.is_cpu() {
        "cpu"
    } else if d.is_cuda() {
        "cuda"
    } else if d.is_metal() {
        "metal"
    } else {
        "other"
    }
}

#[async_trait]
impl EmbeddingProvider for CandleEmbedder {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbedError> {
        // Candle's forward pass is blocking compute (CPU or GPU).
        // Move it onto the tokio blocking pool so we don't stall the
        // async scheduler.
        let model = Arc::clone(&self.model);
        let tokenizer = Arc::clone(&self.tokenizer);
        let device = self.device.clone();
        let dim = self.dim;
        let text = text.to_string();
        task::spawn_blocking(move || {
            let tmp = CandleEmbedder {
                model,
                tokenizer,
                device,
                dim,
                model_id: String::new(),
            };
            tmp.embed_blocking(&text)
        })
        .await
        .map_err(|e| EmbedError::Other(format!("spawn_blocking join: {e}")))?
    }

    fn dim(&self) -> usize {
        self.dim
    }

    fn backend(&self) -> &'static str {
        "candle"
    }
}

// Back-compat constructor. Earlier drafts of this crate exposed a
// `new(model_id, dim) -> Self` signature; existing callers in
// `finima-api` (state.rs, bootstrap bins) use it. We keep the shape
// but make it actually load the model. Callers wanting non-panicking
// construction should use [`CandleEmbedder::load`].
impl CandleEmbedder {
    pub fn new(model_id: impl Into<String>, dim: usize) -> Self {
        let model_id = model_id.into();
        match Self::load(model_id.clone(), dim) {
            Ok(e) => e,
            Err(e) => panic!(
                "CandleEmbedder::new({model_id}, {dim}) failed: {e}; \
                 use CandleEmbedder::load for non-panicking construction"
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_kind_labels() {
        assert_eq!(device_kind(&Device::Cpu), "cpu");
    }

    // Real model download: heavy, network-dependent. Run locally with:
    //   cargo test -p finima-embed --features candle -- --ignored
    #[tokio::test]
    #[ignore]
    async fn loads_minilm_and_embeds_unit_vector() {
        let e = CandleEmbedder::load("sentence-transformers/all-MiniLM-L6-v2", 384)
            .expect("load MiniLM");
        let v = e.embed("hello world").await.expect("embed");
        assert_eq!(v.len(), 384);
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-4, "not unit-norm: norm={norm}");
    }
}
