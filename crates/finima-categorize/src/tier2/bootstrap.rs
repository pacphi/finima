//! Explicit, observable bootstrap path for Tier 2 stores (ADR-012 Phase 3).
//!
//! Cold-start behavior is a first-class concern for the RuVector backend:
//! a fresh HNSW index is useless until it has a seed population of
//! previously-categorized transactions. Rather than building that seeding
//! into engine construction (opaque, hard to reason about, slow on first
//! boot for large portfolios), this module exposes an explicit entry point
//! that callers invoke on startup and whose progress / result is returned
//! as a [`BootstrapReport`].
//!
//! The report surfaces:
//! * How many labeled examples were offered.
//! * How many were actually inserted vs. skipped / rejected.
//! * Wall-clock duration.
//! * Any errors encountered (captured, not panicked).
//!
//! This type is backend-agnostic: it works for the Jaccard
//! [`super::EmbeddingStore`] and for the RuVector store gated behind the
//! `sona` feature.

use std::fmt;
use std::time::{Duration, Instant};

use super::SemanticVectorIngest;

/// A single labeled example handed to the bootstrap routine.
///
/// `vector` is optional: callers with a precomputed embedding can pass it
/// through to avoid re-embedding. The Jaccard backend ignores it; the
/// RuVector backend uses it directly.
#[derive(Debug, Clone)]
pub struct LabeledExample {
    pub description: String,
    pub category: String,
    pub subcategory: String,
    pub confidence: f64,
    pub vector: Option<Vec<f32>>,
}

/// Structured result of a bootstrap run. Safe to log and/or emit as a
/// metric. All counters are monotonically non-decreasing during the run.
#[derive(Debug, Clone, Default)]
pub struct BootstrapReport {
    /// Total examples seen by the iterator (before any filtering).
    pub offered: usize,
    /// Examples successfully ingested (counts toward the store's size).
    pub inserted: usize,
    /// Examples skipped because of the `max_examples` cap.
    pub skipped_cap: usize,
    /// Examples the backend refused (dim mismatch, empty description, etc.).
    pub rejected: usize,
    /// Wall-clock duration of the bootstrap pass.
    pub elapsed: Duration,
}

impl fmt::Display for BootstrapReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Tier2 bootstrap: offered={} inserted={} skipped_cap={} rejected={} elapsed={:.2?}",
            self.offered, self.inserted, self.skipped_cap, self.rejected, self.elapsed
        )
    }
}

/// Opaque error wrapper so callers can match on a stable surface without
/// depending on which backend produced the failure. For Phase 1 the only
/// failure mode is upstream-supplied: the iterator itself can yield a
/// `Result` whose `Err` arm terminates bootstrap early.
#[derive(Debug)]
pub enum BootstrapError {
    /// Upstream iterator returned an error; the bootstrap run aborted
    /// early. The partial report is still returned alongside this error.
    Source(Box<dyn std::error::Error + Send + Sync>),
}

impl fmt::Display for BootstrapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source(e) => write!(f, "bootstrap source error: {e}"),
        }
    }
}

impl std::error::Error for BootstrapError {}

/// Drive an explicit Tier 2 bootstrap pass.
///
/// `store` is the target `SemanticCategorizer` (Jaccard or RuVector).
/// `seed` yields `LabeledExample`s; typical producers are:
///   * `sqlx::query_as!(...).fetch_all(...)` result mapped to `LabeledExample`
///   * an async stream wrapped with `futures::executor::block_on_stream`
///
/// `max_examples == 0` means unbounded.
///
/// This function is synchronous by design: Tier 2 stores are `!Sync` for
/// insertions, so callers that want this to run in the background should
/// spawn it onto a dedicated `tokio::task::spawn_blocking`.
pub fn bootstrap_semantic<S, I>(
    store: &mut S,
    seed: I,
    max_examples: usize,
) -> (BootstrapReport, Option<BootstrapError>)
where
    S: SemanticVectorIngest + ?Sized,
    I: IntoIterator<Item = LabeledExample>,
{
    let t0 = Instant::now();
    let mut report = BootstrapReport::default();

    for example in seed {
        report.offered += 1;

        if max_examples > 0 && report.inserted >= max_examples {
            report.skipped_cap += 1;
            continue;
        }

        if example.description.trim().is_empty() {
            report.rejected += 1;
            continue;
        }

        let accepted = store.learn_with_vector(
            &example.description,
            &example.category,
            &example.subcategory,
            example.confidence,
            example.vector.as_deref(),
        );

        if accepted {
            report.inserted += 1;
        } else {
            report.rejected += 1;
        }
    }

    report.elapsed = t0.elapsed();
    tracing::info!(
        offered = report.offered,
        inserted = report.inserted,
        skipped_cap = report.skipped_cap,
        rejected = report.rejected,
        elapsed_ms = report.elapsed.as_millis() as u64,
        "Tier 2 bootstrap complete"
    );
    (report, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tier2::{EmbeddingStore, SemanticCategorizer};

    fn ex(desc: &str, cat: &str, sub: &str) -> LabeledExample {
        LabeledExample {
            description: desc.into(),
            category: cat.into(),
            subcategory: sub.into(),
            confidence: 0.95,
            vector: None,
        }
    }

    #[test]
    fn bootstrap_jaccard_counts_correctly() {
        let mut store = EmbeddingStore::new(0.65);
        let seed = vec![
            ex("STARBUCKS COFFEE", "food_dining", "coffee_shops"),
            ex("SHELL GAS STATION", "transportation", "gas_fuel"),
            ex("   ", "junk", "junk"),
        ];

        let (report, err) = bootstrap_semantic(&mut store, seed, 0);
        assert!(err.is_none());
        assert_eq!(report.offered, 3);
        assert_eq!(report.inserted, 2);
        assert_eq!(report.rejected, 1);
        assert_eq!(report.skipped_cap, 0);
        assert_eq!(store.index_size(), 2);
    }

    #[test]
    fn bootstrap_respects_cap() {
        let mut store = EmbeddingStore::new(0.65);
        let seed: Vec<LabeledExample> = (0..10)
            .map(|i| ex(&format!("MERCHANT {i}"), "misc", "misc"))
            .collect();

        let (report, _err) = bootstrap_semantic(&mut store, seed, 3);
        assert_eq!(report.inserted, 3);
        assert_eq!(report.skipped_cap, 7);
        assert_eq!(store.index_size(), 3);
    }
}
