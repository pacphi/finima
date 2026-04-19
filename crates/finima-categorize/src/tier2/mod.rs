pub mod bootstrap;
pub mod embedding_store;

#[cfg(feature = "sona")]
pub mod ruvector_store;

pub use bootstrap::{bootstrap_semantic, BootstrapError, BootstrapReport, LabeledExample};
pub use embedding_store::EmbeddingStore;

#[cfg(feature = "sona")]
pub use ruvector_store::RuVectorEmbeddingStore;

use crate::types::{CategoryAssignment, UncategorizedTransaction};

/// Trait for semantic similarity-based categorization (Tier 2).
///
/// Implementations maintain an index of previously-categorized transaction
/// descriptions and attempt to match new transactions by similarity.
pub trait SemanticCategorizer: Send + Sync {
    /// Attempt to categorize a transaction by finding similar previously-categorized descriptions.
    fn categorize(&self, txn: &UncategorizedTransaction) -> Option<CategoryAssignment>;

    /// Add a labeled example to the index for future similarity matching.
    fn learn(&mut self, description: &str, category: &str, subcategory: &str, confidence: f64);

    /// Number of examples in the index.
    fn index_size(&self) -> usize;
}

/// Extension trait that adds embedding-aware ingest used by the RuVector
/// backend. Implementations that don't use vectors (e.g., the Jaccard
/// [`EmbeddingStore`]) ignore the vector argument and fall back to
/// [`SemanticCategorizer::learn`].
///
/// Every Tier 2 backend implements this explicitly. No blanket impl — we
/// want each backend to make its own decision about whether a missing
/// vector should be rejected or tolerated.
pub trait SemanticVectorIngest: SemanticCategorizer {
    /// Ingest a labeled example with a caller-supplied embedding. The
    /// expected dimensionality comes from the backend's `Tier2Config::dim`;
    /// mismatched vectors are rejected (returned as `false`).
    fn learn_with_vector(
        &mut self,
        description: &str,
        category: &str,
        subcategory: &str,
        confidence: f64,
        vector: Option<&[f32]>,
    ) -> bool;
}
