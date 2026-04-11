//! Model download utilities for pre-fetching GGUF models from HuggingFace Hub.
//!
//! Requires the `candle` feature flag.
//!
//! This module provides helpers to download, cache-check, and resolve
//! GGUF model files from HuggingFace Hub so that Candle inference can
//! start immediately without a cold-download on first request.

#![cfg(feature = "candle")]

use std::path::PathBuf;

use hf_hub::api::sync::ApiBuilder;
use hf_hub::{Cache, Repo, RepoType};
use tracing::{debug, info, warn};

use crate::error::LlmError;
use crate::hardware::{detect_hardware, resolve_model, ModelSelection};

// ---------------------------------------------------------------------------
// Model specification
// ---------------------------------------------------------------------------

/// A predefined model configuration describing a downloadable GGUF variant.
#[derive(Debug, Clone)]
pub struct ModelSpec {
    /// HuggingFace repository identifier (e.g. `"google/gemma-4-26B-A4B-it"`).
    pub repo_id: &'static str,
    /// Filename of the GGUF file within the repository.
    pub filename: &'static str,
    /// Human-readable description of the variant.
    pub description: &'static str,
    /// Approximate download size in gigabytes.
    pub size_gb: f64,
}

/// Returns the list of predefined Gemma 4 GGUF model variants supported by
/// the hardware-detection logic.
pub fn available_models() -> Vec<ModelSpec> {
    vec![
        ModelSpec {
            repo_id: "google/gemma-4-26B-A4B-it",
            filename: "gemma-4-26B-A4B-it-Q4_K_M.gguf",
            description: "Gemma 4 26B MoE (A4B) — best quality, requires 16+ GB VRAM",
            size_gb: 15.0,
        },
        ModelSpec {
            repo_id: "google/gemma-4-E4B-it",
            filename: "gemma-4-E4B-it-Q4_K_M.gguf",
            description: "Gemma 4 E4B — balanced quality/speed, requires 8+ GB VRAM",
            size_gb: 3.0,
        },
        ModelSpec {
            repo_id: "google/gemma-4-E2B-it",
            filename: "gemma-4-E2B-it-Q4_K_M.gguf",
            description: "Gemma 4 E2B — lightweight, suitable for < 8 GB VRAM",
            size_gb: 1.8,
        },
    ]
}

// ---------------------------------------------------------------------------
// Cache helpers
// ---------------------------------------------------------------------------

/// Returns the default HuggingFace Hub cache directory.
///
/// Respects the `HF_HOME` environment variable; falls back to
/// `~/.cache/huggingface/hub`.
pub fn default_cache_dir() -> PathBuf {
    Cache::default().path().clone()
}

/// Check whether a model's GGUF file is already present in the local
/// HuggingFace cache.
///
/// This performs a purely local filesystem check — it does not contact the
/// Hub.  Returns `true` when the file can be found in the cache's snapshot
/// directory for the given repo.
pub fn is_model_cached(repo_id: &str, filename: &str) -> bool {
    let cache = Cache::default();
    let repo = Repo::new(repo_id.to_string(), RepoType::Model);
    let cache_repo = cache.repo(repo);
    cache_repo.get(filename).is_some()
}

// ---------------------------------------------------------------------------
// Download
// ---------------------------------------------------------------------------

/// Download a GGUF model file from HuggingFace Hub.
///
/// If the file is already cached locally the cached path is returned
/// immediately without re-downloading.  Progress is reported via `tracing`
/// log events.
///
/// # Errors
///
/// Returns [`LlmError::ModelLoad`] when the Hub API cannot be initialised
/// or the download itself fails.
pub fn download_model(repo_id: &str, filename: &str) -> Result<PathBuf, LlmError> {
    // Fast-path: check cache first.
    if is_model_cached(repo_id, filename) {
        let cache = Cache::default();
        let repo = Repo::new(repo_id.to_string(), RepoType::Model);
        let path = cache
            .repo(repo)
            .get(filename)
            .expect("is_model_cached returned true but get returned None");
        info!(
            repo_id,
            filename,
            path = %path.display(),
            "Model already cached, skipping download"
        );
        return Ok(path);
    }

    info!(
        repo_id,
        filename, "Starting model download from HuggingFace Hub"
    );

    // Disable the built-in progress bar — we log progress via tracing instead.
    let api = ApiBuilder::new()
        .with_progress(true)
        .build()
        .map_err(|e| LlmError::ModelLoad(format!("failed to create HF Hub API client: {e}")))?;

    let api_repo = api.model(repo_id.to_string());

    debug!(repo_id, filename, "Fetching file from Hub (may download)");

    let path = api_repo.get(filename).map_err(|e| {
        LlmError::ModelLoad(format!("failed to download {filename} from {repo_id}: {e}"))
    })?;

    info!(
        repo_id,
        filename,
        path = %path.display(),
        "Model download complete"
    );

    Ok(path)
}

/// Download the default model based on automatic hardware detection.
///
/// Calls [`detect_hardware`] and [`resolve_model`] with `"auto"` to pick
/// the best Gemma 4 variant for the current machine, then downloads (or
/// verifies the cache for) the corresponding GGUF file.
///
/// # Errors
///
/// Returns [`LlmError::ModelLoad`] on download failure or
/// [`LlmError::HardwareDetection`] if the resolved model does not
/// correspond to a known GGUF file.
pub fn download_default_model() -> Result<PathBuf, LlmError> {
    let profile = detect_hardware();
    let selection = resolve_model(&profile, "auto");

    match selection {
        ModelSelection::Auto {
            model_id,
            gguf_file,
            reason,
        } => {
            info!(model_id, gguf_file, reason, "Resolved model for hardware");
            download_model(&model_id, &gguf_file)
        }
        ModelSelection::Explicit(model_id) => {
            // This branch should not happen with "auto", but handle it
            // gracefully by looking up the spec table.
            warn!(
                model_id,
                "resolve_model returned Explicit for 'auto'; attempting spec lookup"
            );
            let specs = available_models();
            let spec = specs
                .iter()
                .find(|s| s.repo_id == model_id)
                .ok_or_else(|| {
                    LlmError::HardwareDetection(format!(
                        "no known GGUF spec for explicit model '{model_id}'"
                    ))
                })?;
            download_model(spec.repo_id, spec.filename)
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn available_models_returns_three_variants() {
        let models = available_models();
        assert_eq!(models.len(), 3, "expected exactly 3 model variants");
    }

    #[test]
    fn available_models_have_valid_fields() {
        for spec in available_models() {
            assert!(!spec.repo_id.is_empty(), "repo_id must not be empty");
            assert!(
                spec.filename.ends_with(".gguf"),
                "filename '{}' should end with .gguf",
                spec.filename
            );
            assert!(
                !spec.description.is_empty(),
                "description must not be empty"
            );
            assert!(spec.size_gb > 0.0, "size_gb must be positive");
        }
    }

    #[test]
    fn available_models_contain_expected_repo_ids() {
        let models = available_models();
        let ids: Vec<&str> = models.iter().map(|m| m.repo_id).collect();
        assert!(ids.contains(&"google/gemma-4-26B-A4B-it"));
        assert!(ids.contains(&"google/gemma-4-E4B-it"));
        assert!(ids.contains(&"google/gemma-4-E2B-it"));
    }

    #[test]
    fn default_cache_dir_is_absolute() {
        let dir = default_cache_dir();
        assert!(dir.is_absolute(), "cache dir should be an absolute path");
    }

    #[test]
    fn default_cache_dir_ends_with_hub() {
        let dir = default_cache_dir();
        assert!(
            dir.ends_with("hub"),
            "default cache dir should end with 'hub', got: {}",
            dir.display()
        );
    }

    #[test]
    fn is_model_cached_returns_false_for_nonexistent() {
        // A repo that almost certainly does not exist in the local cache.
        assert!(!is_model_cached(
            "nonexistent-org/nonexistent-model",
            "nonexistent.gguf"
        ));
    }
}
