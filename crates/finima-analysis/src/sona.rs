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

// When the `sona` feature is enabled, this module provides a RuVector-backed
// implementation of `FlowPatternMatcher` using HNSW for nearest-neighbor
// search and SONA for self-learning adaptation.
//
// TODO: Implement when `ruvector` crate is added as a dependency.
//
// ```rust
// #[cfg(feature = "sona")]
// pub struct RuVectorPatternMatcher {
//     index: ruvector::HnswIndex,
//     sona: ruvector::SonaAdapter,
//     patterns: Vec<FlowPattern>,
// }
// ```

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
