//! RuVector-backed Tier 2 semantic store (ADR-012 / ADR-017).
//!
//! Compiles only when the `sona` feature is enabled. At this feature
//! level we depend on `ruvector-core` (HNSW + SIMD, memory-only) and
//! `ruvector-sona` (ReasoningBank + MicroLoRA). See
//! `docs/spikes/ruvector-phase0.md` for the API decisions this is built on.
//!
//! Phase 1 contract: **bring-your-own vectors.** This store does not
//! embed text itself — callers hand it precomputed f32 unit-vectors of
//! the configured dimensionality. Embedding sourcing (local ONNX,
//! external API, etc.) is intentionally out of scope for Phase 1 and
//! tracked as a Phase 2 decision.
//!
//! The learning loop we implement is **ReasoningBank-only**. MicroLoRA
//! weight adaptation is deferred per the Phase 0d spike finding: at the
//! usage pattern we need (single-step trajectories), MicroLoRA stays at
//! its zero-initialised state and `apply_micro_lora` returns zeros. That
//! is fine because the retrieval signal in `find_patterns` does not
//! depend on MicroLoRA.

use std::collections::HashMap;

use ruvector_core::types::{DbOptions, HnswConfig};
use ruvector_core::{DistanceMetric, SearchQuery, VectorDB, VectorEntry};
use ruvector_sona::engine::SonaEngineBuilder;
use ruvector_sona::SonaEngine;

use super::{SemanticCategorizer, SemanticVectorIngest};
use crate::config::Tier2Config;
use crate::types::{CategorizationTier, CategoryAssignment, UncategorizedTransaction};

/// HNSW + ReasoningBank Tier 2 backend.
///
/// # Failure model
///
/// Construction returns a `Result`: HNSW allocation can fail for
/// pathological dimensions, and misconfigured `Tier2Config` should surface
/// loudly rather than panicking mid-request. Insertion and search errors
/// are logged and converted to "no match"; the cascade falls through to
/// lower-confidence tiers rather than aborting the user's categorization.
pub struct RuVectorEmbeddingStore {
    db: VectorDB,
    engine: SonaEngine,
    cfg: Tier2Config,
    min_confidence: f64,
    // `db.len()` is fallible and clones metadata; keep a local counter.
    size: usize,
}

/// Error surface for construction. Kept narrow on purpose — runtime errors
/// during `learn` / `categorize` are handled internally and logged.
#[derive(Debug, thiserror::Error)]
pub enum RuVectorStoreError {
    #[error("ruvector VectorDB construction failed: {0}")]
    VectorDb(String),
}

impl RuVectorEmbeddingStore {
    /// Build a fresh store from config. `min_confidence` is the Tier 2 gate
    /// (`CategorizeConfig::semantic_min_confidence`); matches below it are
    /// reported as "no match" and the cascade falls through.
    pub fn new(cfg: Tier2Config, min_confidence: f64) -> Result<Self, RuVectorStoreError> {
        let hnsw = HnswConfig {
            m: cfg.hnsw_m,
            ef_construction: cfg.hnsw_ef_construction,
            ef_search: cfg.hnsw_ef_search,
            ..HnswConfig::default()
        };
        let opts = DbOptions {
            dimensions: cfg.dim,
            distance_metric: DistanceMetric::Cosine,
            storage_path: String::new(), // memory-only: Postgres is the source of truth
            hnsw_config: Some(hnsw),
            quantization: None,
        };

        let db = VectorDB::new(opts).map_err(|e| RuVectorStoreError::VectorDb(e.to_string()))?;

        let engine: SonaEngine = SonaEngineBuilder::new().hidden_dim(cfg.dim).build();

        Ok(Self {
            db,
            engine,
            cfg,
            min_confidence,
            size: 0,
        })
    }

    /// Restore ReasoningBank patterns from a serialized SONA state blob
    /// (see Phase 0c spike). The blob format is whatever
    /// `engine.coordinator().serialize_state()` produced at persist time;
    /// we don't inspect it. Returns the number of patterns reloaded.
    pub fn restore_sona_state(&self, json: &str) -> Result<usize, String> {
        self.engine.coordinator().load_state(json)
    }

    /// Snapshot the current ReasoningBank state for later [`restore_sona_state`].
    pub fn snapshot_sona_state(&self) -> String {
        self.engine.coordinator().serialize_state()
    }

    pub fn cfg(&self) -> &Tier2Config {
        &self.cfg
    }

    fn search_top(&self, vector: &[f32]) -> Option<ruvector_core::SearchResult> {
        if vector.len() != self.cfg.dim {
            tracing::debug!(
                got = vector.len(),
                expected = self.cfg.dim,
                "Tier 2 query vector dimension mismatch"
            );
            return None;
        }

        let query = SearchQuery {
            vector: vector.to_vec(),
            k: 1,
            filter: None,
            ef_search: None,
        };
        match self.db.search(query) {
            Ok(mut hits) => hits.pop(),
            Err(e) => {
                tracing::warn!(error = %e, "Tier 2 HNSW search failed");
                None
            }
        }
    }
}

// Cosine distance in ruvector-core is returned as a *distance* (lower is
// better; 0 = identical, higher = further). Convert to a [0,1]-ish
// similarity for the Tier 2 threshold check.
fn cosine_similarity_from_score(score: f32) -> f64 {
    let s = 1.0 - score as f64;
    s.clamp(0.0, 1.0)
}

fn meta_str<'a>(
    md: &'a Option<HashMap<String, serde_json::Value>>,
    key: &str,
) -> Option<&'a str> {
    md.as_ref()?.get(key)?.as_str()
}

impl SemanticCategorizer for RuVectorEmbeddingStore {
    fn categorize(&self, _txn: &UncategorizedTransaction) -> Option<CategoryAssignment> {
        // Without a caller-supplied embedding we can't probe HNSW. The
        // cascade should route `UncategorizedTransaction`s through
        // `categorize_with_vector` (future) or via an external embedder
        // layer. Returning `None` here keeps the trait satisfied and makes
        // this backend a no-op for text-only callers — Jaccard remains
        // available as the text-only fallback.
        None
    }

    fn learn(&mut self, _description: &str, _category: &str, _subcategory: &str, _confidence: f64) {
        // Vector-less learn is a no-op for this backend; see
        // `learn_with_vector`. We intentionally do not hash the description
        // to synthesize a pseudo-vector: that produces silently wrong
        // retrieval behavior (Phase 0 verified this hazard for the
        // `HashEmbedding` path in ruvector-core).
        tracing::debug!("RuVectorEmbeddingStore::learn called without vector; ignored");
    }

    fn index_size(&self) -> usize {
        self.size
    }
}

impl SemanticVectorIngest for RuVectorEmbeddingStore {
    fn learn_with_vector(
        &mut self,
        description: &str,
        category: &str,
        subcategory: &str,
        confidence: f64,
        vector: Option<&[f32]>,
    ) -> bool {
        let Some(v) = vector else {
            tracing::debug!(
                description,
                "RuVectorEmbeddingStore::learn_with_vector requires a vector; rejecting"
            );
            return false;
        };
        if v.len() != self.cfg.dim {
            tracing::debug!(
                got = v.len(),
                expected = self.cfg.dim,
                description,
                "learn_with_vector dim mismatch"
            );
            return false;
        }

        let mut md = HashMap::new();
        md.insert("desc".into(), serde_json::Value::String(description.to_string()));
        md.insert("category".into(), serde_json::Value::String(category.to_string()));
        md.insert("subcategory".into(), serde_json::Value::String(subcategory.to_string()));
        md.insert(
            "confidence".into(),
            serde_json::json!(confidence),
        );

        let entry = VectorEntry {
            id: None,
            vector: v.to_vec(),
            metadata: Some(md),
        };
        match self.db.insert(entry) {
            Ok(_) => {
                self.size += 1;
                true
            }
            Err(e) => {
                tracing::warn!(error = %e, description, "Tier 2 HNSW insert failed");
                false
            }
        }
    }
}

/// Convenience wrapper: categorize a transaction given its precomputed
/// embedding. This is the primary query path for vector-based Tier 2
/// backends.
pub fn categorize_with_vector(
    store: &RuVectorEmbeddingStore,
    txn: &UncategorizedTransaction,
    vector: &[f32],
) -> Option<CategoryAssignment> {
    let hit = store.search_top(vector)?;
    let sim = cosine_similarity_from_score(hit.score);
    if sim < store.min_confidence {
        return None;
    }
    let md = hit.metadata;
    let category = meta_str(&md, "category").unwrap_or("").to_string();
    let subcategory = meta_str(&md, "subcategory").unwrap_or("").to_string();
    if category.is_empty() {
        return None;
    }

    Some(CategoryAssignment {
        transaction_id: txn.id,
        category,
        subcategory,
        merchant_name: String::new(),
        confidence: sim,
        source_tier: CategorizationTier::SemanticSearch,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use rust_decimal::Decimal;
    use uuid::Uuid;

    fn unit_vec(seed: u64, dim: usize) -> Vec<f32> {
        let mut v = Vec::with_capacity(dim);
        let mut s = seed.wrapping_mul(2654435761);
        for _ in 0..dim {
            s = s
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            v.push(((s >> 33) as f32) / (u32::MAX as f32) - 0.5);
        }
        let n: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-9);
        for x in &mut v {
            *x /= n;
        }
        v
    }

    fn cfg_dim(dim: usize) -> Tier2Config {
        Tier2Config {
            dim,
            ..Tier2Config::default()
        }
    }

    #[test]
    fn construct_and_ingest() {
        let mut store = RuVectorEmbeddingStore::new(cfg_dim(64), 0.0).expect("build");
        assert_eq!(store.index_size(), 0);

        let v = unit_vec(1, 64);
        let ok = store.learn_with_vector("STARBUCKS", "food_dining", "coffee_shops", 0.95, Some(&v));
        assert!(ok);
        assert_eq!(store.index_size(), 1);
    }

    #[test]
    fn wrong_dim_rejected() {
        let mut store = RuVectorEmbeddingStore::new(cfg_dim(64), 0.0).expect("build");
        let v = unit_vec(1, 32); // wrong
        let ok = store.learn_with_vector("X", "a", "b", 0.9, Some(&v));
        assert!(!ok);
        assert_eq!(store.index_size(), 0);
    }

    #[test]
    fn search_returns_nearest_label_above_threshold() {
        let dim = 64;
        let mut store = RuVectorEmbeddingStore::new(cfg_dim(dim), 0.0).expect("build");
        let v0 = unit_vec(10, dim);
        let v1 = unit_vec(11, dim);
        store.learn_with_vector("STARBUCKS COFFEE", "food_dining", "coffee_shops", 0.95, Some(&v0));
        store.learn_with_vector("SHELL GAS", "transportation", "gas_fuel", 0.90, Some(&v1));

        let txn = UncategorizedTransaction {
            id: Uuid::new_v4(),
            description: "irrelevant — using vector".into(),
            amount: Decimal::new(-575, 2),
            date: NaiveDate::from_ymd_opt(2026, 1, 15).unwrap(),
            mcc: None,
        };
        let result = categorize_with_vector(&store, &txn, &v0).expect("match");
        assert_eq!(result.category, "food_dining");
        assert_eq!(result.subcategory, "coffee_shops");
        assert!(result.confidence > 0.99); // identical vector
    }

    #[test]
    fn below_threshold_returns_none() {
        let dim = 64;
        let mut store = RuVectorEmbeddingStore::new(cfg_dim(dim), 0.99).expect("build");
        let v0 = unit_vec(1, dim);
        let v1 = unit_vec(2, dim);
        store.learn_with_vector("SEED", "a", "b", 0.9, Some(&v0));

        let txn = UncategorizedTransaction {
            id: Uuid::new_v4(),
            description: "query".into(),
            amount: Decimal::new(0, 0),
            date: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            mcc: None,
        };
        // v1 is a different random unit vector — cosine similarity will
        // be small, well below 0.99.
        let result = categorize_with_vector(&store, &txn, &v1);
        assert!(result.is_none());
    }

    #[test]
    fn sona_state_round_trips() {
        let store = RuVectorEmbeddingStore::new(cfg_dim(64), 0.0).expect("build");
        let blob = store.snapshot_sona_state();
        let n = store.restore_sona_state(&blob).expect("restore");
        // Fresh engine has no patterns yet, but round-trip must succeed.
        assert_eq!(n, 0);
    }
}
