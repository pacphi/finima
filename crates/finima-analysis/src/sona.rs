//! SONA (Self-Organizing Neural Architecture) integration for flow detection.
//!
//! Provides semantic matching capabilities to enhance the heuristic-based
//! flow detection in [`crate::flows`]. When the `sona` feature is enabled,
//! this module uses RuVector's HNSW index and SONA self-learning to:
//!
//! 1. Resolve one-sided flows by inferring target accounts from description
//!    similarity to previously confirmed flows.
//! 2. Detect transfer-like transactions that don't match the static keyword
//!    list but are semantically similar to known transfers.
//! 3. Learn from user confirmations/dismissals to improve accuracy over time.
//!
//! When the `sona` feature is NOT enabled, all functions return empty results
//! and the system falls back to heuristic-only detection.

use uuid::Uuid;

/// A learned flow pattern stored in the HNSW index.
#[derive(Debug, Clone)]
pub struct FlowPattern {
    pub description: String,
    pub source_account_id: Uuid,
    pub target_account_id: Uuid,
    pub confidence: f64,
    pub match_count: u32,
}

/// Result of a SONA-based target account inference.
#[derive(Debug, Clone)]
pub struct InferredTarget {
    pub target_account_id: Uuid,
    pub confidence: f64,
    pub matched_pattern: String,
}

/// Trait for SONA-based flow pattern matching.
///
/// Implementations can use RuVector's HNSW index for production, or a stub
/// for testing and environments where SONA is not available.
pub trait FlowPatternMatcher: Send + Sync {
    /// Given a transaction description from a one-sided flow, infer the most
    /// likely target account by searching for similar confirmed flow patterns.
    ///
    /// Returns `None` if no match exceeds the confidence threshold.
    fn infer_target(
        &self,
        description: &str,
        source_account_id: Uuid,
        min_confidence: f64,
    ) -> Option<InferredTarget>;

    /// Store a confirmed flow pattern for future matching.
    fn store_pattern(&mut self, pattern: FlowPattern);

    /// Record negative feedback (dismissed flow) to reduce confidence of
    /// similar patterns.
    fn record_dismissal(&mut self, description: &str, source_account_id: Uuid);

    /// Return the number of stored patterns.
    fn pattern_count(&self) -> usize;
}

/// Stub implementation that returns no matches.
///
/// Used when SONA/RuVector is not enabled or during initial bootstrap
/// before any patterns have been learned.
pub struct StubPatternMatcher;

impl FlowPatternMatcher for StubPatternMatcher {
    fn infer_target(
        &self,
        _description: &str,
        _source_account_id: Uuid,
        _min_confidence: f64,
    ) -> Option<InferredTarget> {
        None
    }

    fn store_pattern(&mut self, _pattern: FlowPattern) {}

    fn record_dismissal(&mut self, _description: &str, _source_account_id: Uuid) {}

    fn pattern_count(&self) -> usize {
        0
    }
}

// ---------------------------------------------------------------------------
// RuVector-backed implementation (behind `sona` feature flag)
// ---------------------------------------------------------------------------

#[cfg(feature = "sona")]
pub use ruvector_backend::{
    RuVectorPatternMatcher, RuVectorPatternMatcherConfig, RuVectorPatternMatcherError,
};

#[cfg(feature = "sona")]
mod ruvector_backend {
    //! HNSW + ReasoningBank flow-pattern matcher. ADR-017.
    //!
    //! Contract: **bring-your-own vectors.** Callers supply a precomputed
    //! unit-vector embedding of the transaction description; this matcher
    //! stores it in a `VectorDB` HNSW index with metadata carrying the
    //! `(source_account_id, target_account_id)` pair, and answers k-NN
    //! queries filtered to the same source account. The trait methods
    //! (`infer_target`, `store_pattern`, `record_dismissal`) are
    //! vector-less and act as no-ops so the backend can be swapped in
    //! without changing the existing flow-detection code paths; callers
    //! with a vector call `infer_target_with_vector` /
    //! `store_pattern_with_vector` directly.
    //!
    //! Per Phase 0d: MicroLoRA adaptation is deferred — we only exercise
    //! ReasoningBank pattern retrieval, which is the load-bearing signal.
    //! ReasoningBank state survives restart via
    //! `snapshot_sona_state` / `restore_sona_state`; LoRA weights do not
    //! (no import API in ruvector-sona 0.1.9).

    use std::collections::HashMap;

    use ruvector_core::types::{DbOptions, HnswConfig};
    use ruvector_core::{DistanceMetric, SearchQuery, VectorDB, VectorEntry};
    use ruvector_sona::engine::SonaEngineBuilder;
    use ruvector_sona::SonaEngine;
    use uuid::Uuid;

    use super::{FlowPattern, FlowPatternMatcher, InferredTarget};

    #[derive(Debug, Clone)]
    pub struct RuVectorPatternMatcherConfig {
        pub dim: usize,
        pub hnsw_m: usize,
        pub hnsw_ef_construction: usize,
        pub hnsw_ef_search: usize,
    }

    impl Default for RuVectorPatternMatcherConfig {
        fn default() -> Self {
            Self {
                dim: 384,
                hnsw_m: 32,
                hnsw_ef_construction: 200,
                hnsw_ef_search: 100,
            }
        }
    }

    #[derive(Debug, thiserror::Error)]
    pub enum RuVectorPatternMatcherError {
        #[error("ruvector VectorDB construction failed: {0}")]
        VectorDb(String),
    }

    pub struct RuVectorPatternMatcher {
        db: VectorDB,
        engine: SonaEngine,
        cfg: RuVectorPatternMatcherConfig,
        size: usize,
    }

    impl RuVectorPatternMatcher {
        pub fn new(cfg: RuVectorPatternMatcherConfig) -> Result<Self, RuVectorPatternMatcherError> {
            let hnsw = HnswConfig {
                m: cfg.hnsw_m,
                ef_construction: cfg.hnsw_ef_construction,
                ef_search: cfg.hnsw_ef_search,
                ..HnswConfig::default()
            };
            let opts = DbOptions {
                dimensions: cfg.dim,
                distance_metric: DistanceMetric::Cosine,
                storage_path: String::new(),
                hnsw_config: Some(hnsw),
                quantization: None,
            };
            let db = VectorDB::new(opts)
                .map_err(|e| RuVectorPatternMatcherError::VectorDb(e.to_string()))?;
            let engine: SonaEngine = SonaEngineBuilder::new().hidden_dim(cfg.dim).build();
            Ok(Self {
                db,
                engine,
                cfg,
                size: 0,
            })
        }

        /// Primary ingest path: store a confirmed flow pattern together
        /// with a caller-supplied embedding. Rejects dim-mismatched
        /// vectors (returned as `false`).
        pub fn store_pattern_with_vector(&mut self, pattern: FlowPattern, vector: &[f32]) -> bool {
            if vector.len() != self.cfg.dim {
                tracing::debug!(
                    got = vector.len(),
                    expected = self.cfg.dim,
                    "store_pattern_with_vector dim mismatch"
                );
                return false;
            }

            let mut md = HashMap::new();
            md.insert(
                "description".into(),
                serde_json::Value::String(pattern.description.clone()),
            );
            md.insert(
                "source_account_id".into(),
                serde_json::Value::String(pattern.source_account_id.to_string()),
            );
            md.insert(
                "target_account_id".into(),
                serde_json::Value::String(pattern.target_account_id.to_string()),
            );
            md.insert("confidence".into(), serde_json::json!(pattern.confidence));
            md.insert("match_count".into(), serde_json::json!(pattern.match_count));

            let entry = VectorEntry {
                id: None,
                vector: vector.to_vec(),
                metadata: Some(md),
            };
            match self.db.insert(entry) {
                Ok(_) => {
                    self.size += 1;
                    true
                }
                Err(e) => {
                    tracing::warn!(error = %e, "flow-pattern HNSW insert failed");
                    false
                }
            }
        }

        /// k-NN probe with a caller-supplied query vector. Returns the
        /// best hit that (a) is keyed on the same `source_account_id`,
        /// and (b) exceeds `min_confidence` after converting cosine
        /// distance to similarity.
        pub fn infer_target_with_vector(
            &self,
            _description: &str,
            source_account_id: Uuid,
            query_vector: &[f32],
            min_confidence: f64,
        ) -> Option<InferredTarget> {
            if query_vector.len() != self.cfg.dim {
                tracing::debug!(
                    got = query_vector.len(),
                    expected = self.cfg.dim,
                    "infer_target_with_vector dim mismatch"
                );
                return None;
            }

            let q = SearchQuery {
                vector: query_vector.to_vec(),
                k: 5,
                filter: None,
                ef_search: None,
            };
            let hits = match self.db.search(q) {
                Ok(h) => h,
                Err(e) => {
                    tracing::warn!(error = %e, "flow-pattern HNSW search failed");
                    return None;
                }
            };

            let source_str = source_account_id.to_string();
            hits.into_iter()
                .filter_map(|hit| {
                    let md = hit.metadata.as_ref()?;
                    let src = md.get("source_account_id")?.as_str()?;
                    if src != source_str {
                        return None;
                    }
                    let target = md.get("target_account_id")?.as_str()?;
                    let target_uuid = Uuid::parse_str(target).ok()?;
                    let matched = md
                        .get("description")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let sim = cosine_similarity_from_score(hit.score);
                    Some(InferredTarget {
                        target_account_id: target_uuid,
                        confidence: sim,
                        matched_pattern: matched,
                    })
                })
                .find(|t| t.confidence >= min_confidence)
        }

        /// Snapshot the current ReasoningBank state for later
        /// `restore_sona_state`. See Phase 0c spike.
        pub fn snapshot_sona_state(&self) -> String {
            self.engine.coordinator().serialize_state()
        }

        /// Restore ReasoningBank patterns from a previous `snapshot_sona_state`
        /// blob. Returns the number of patterns reloaded.
        pub fn restore_sona_state(&self, json: &str) -> Result<usize, String> {
            self.engine.coordinator().load_state(json)
        }

        pub fn cfg(&self) -> &RuVectorPatternMatcherConfig {
            &self.cfg
        }
    }

    impl FlowPatternMatcher for RuVectorPatternMatcher {
        fn infer_target(
            &self,
            _description: &str,
            _source_account_id: Uuid,
            _min_confidence: f64,
        ) -> Option<InferredTarget> {
            None
        }

        fn store_pattern(&mut self, _pattern: FlowPattern) {
            tracing::debug!("RuVectorPatternMatcher::store_pattern called without vector; ignored");
        }

        fn record_dismissal(&mut self, _description: &str, _source_account_id: Uuid) {}

        fn pattern_count(&self) -> usize {
            self.size
        }
    }

    fn cosine_similarity_from_score(score: f32) -> f64 {
        let s = 1.0 - score as f64;
        s.clamp(0.0, 1.0)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

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

        fn cfg_dim(dim: usize) -> RuVectorPatternMatcherConfig {
            RuVectorPatternMatcherConfig {
                dim,
                ..RuVectorPatternMatcherConfig::default()
            }
        }

        fn pattern(desc: &str, source: Uuid, target: Uuid) -> FlowPattern {
            FlowPattern {
                description: desc.into(),
                source_account_id: source,
                target_account_id: target,
                confidence: 0.95,
                match_count: 1,
            }
        }

        #[test]
        fn construct_and_ingest() {
            let mut m = RuVectorPatternMatcher::new(cfg_dim(64)).expect("build");
            assert_eq!(m.pattern_count(), 0);
            let src = Uuid::new_v4();
            let dst = Uuid::new_v4();
            let v = unit_vec(1, 64);
            assert!(m.store_pattern_with_vector(pattern("AUTOPAY AMEX", src, dst), &v));
            assert_eq!(m.pattern_count(), 1);
        }

        #[test]
        fn wrong_dim_store_rejected() {
            let mut m = RuVectorPatternMatcher::new(cfg_dim(64)).expect("build");
            let v = unit_vec(1, 32);
            let ok = m.store_pattern_with_vector(pattern("X", Uuid::nil(), Uuid::nil()), &v);
            assert!(!ok);
            assert_eq!(m.pattern_count(), 0);
        }

        #[test]
        fn wrong_dim_query_returns_none() {
            let m = RuVectorPatternMatcher::new(cfg_dim(64)).expect("build");
            let v = unit_vec(1, 32);
            let out = m.infer_target_with_vector("AUTOPAY", Uuid::new_v4(), &v, 0.0);
            assert!(out.is_none());
        }

        #[test]
        fn infer_target_with_vector_returns_same_source_match() {
            let dim = 64;
            let mut m = RuVectorPatternMatcher::new(cfg_dim(dim)).expect("build");
            let src = Uuid::new_v4();
            let dst = Uuid::new_v4();
            let v = unit_vec(42, dim);
            m.store_pattern_with_vector(pattern("AUTOPAY AMEX GOLD", src, dst), &v);

            let result = m
                .infer_target_with_vector("AUTOPAY AMEX GOLD", src, &v, 0.0)
                .expect("match");
            assert_eq!(result.target_account_id, dst);
            assert!(result.confidence > 0.99);
            assert_eq!(result.matched_pattern, "AUTOPAY AMEX GOLD");
        }

        #[test]
        fn infer_target_ignores_other_source_accounts() {
            let dim = 64;
            let mut m = RuVectorPatternMatcher::new(cfg_dim(dim)).expect("build");
            let src_a = Uuid::new_v4();
            let src_b = Uuid::new_v4();
            let dst = Uuid::new_v4();
            let v = unit_vec(7, dim);
            m.store_pattern_with_vector(pattern("XFER", src_a, dst), &v);

            // Same vector, different source — should not match.
            let result = m.infer_target_with_vector("XFER", src_b, &v, 0.0);
            assert!(result.is_none());
        }

        #[test]
        fn infer_target_below_threshold_is_none() {
            let dim = 64;
            let mut m = RuVectorPatternMatcher::new(cfg_dim(dim)).expect("build");
            let src = Uuid::new_v4();
            let dst = Uuid::new_v4();
            let stored = unit_vec(1, dim);
            let query = unit_vec(2, dim); // different random vector
            m.store_pattern_with_vector(pattern("X", src, dst), &stored);

            let result = m.infer_target_with_vector("X", src, &query, 0.99);
            assert!(result.is_none());
        }

        #[test]
        fn sona_state_round_trips() {
            let m = RuVectorPatternMatcher::new(cfg_dim(64)).expect("build");
            let blob = m.snapshot_sona_state();
            let n = m.restore_sona_state(&blob).expect("restore");
            assert_eq!(n, 0);
        }

        #[test]
        fn trait_methods_are_safe_noops() {
            let mut m = RuVectorPatternMatcher::new(cfg_dim(64)).expect("build");
            let src = Uuid::new_v4();
            let dst = Uuid::new_v4();
            assert!(m.infer_target("X", src, 0.5).is_none());
            m.store_pattern(pattern("X", src, dst));
            m.record_dismissal("X", src);
            assert_eq!(m.pattern_count(), 0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stub_returns_none() {
        let matcher = StubPatternMatcher;
        let result = matcher.infer_target("AUTOPAY AMEX GOLD", Uuid::nil(), 0.8);
        assert!(result.is_none());
    }

    #[test]
    fn stub_pattern_count_zero() {
        let matcher = StubPatternMatcher;
        assert_eq!(matcher.pattern_count(), 0);
    }
}
