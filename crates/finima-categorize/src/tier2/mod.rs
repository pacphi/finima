pub mod embedding_store;

pub use embedding_store::EmbeddingStore;

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
