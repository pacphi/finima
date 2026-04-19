use std::collections::HashSet;

use super::{SemanticCategorizer, SemanticVectorIngest};
use crate::types::{CategorizationTier, CategoryAssignment, UncategorizedTransaction};

/// A single stored example with pre-computed character n-grams.
#[allow(dead_code)]
struct EmbeddingEntry {
    /// The normalized description text, retained for debugging and persistence.
    description: String,
    ngrams: HashSet<String>,
    category: String,
    subcategory: String,
    /// The confidence of the original labeling, retained for weighted scoring.
    confidence: f64,
}

/// Lightweight in-memory embedding store using character n-gram Jaccard
/// similarity for fast semantic-like matching.
///
/// This serves as the Tier 2 backend and can be swapped for a vector
/// database (e.g., RuVector HNSW) later via the [`SemanticCategorizer`] trait.
pub struct EmbeddingStore {
    entries: Vec<EmbeddingEntry>,
    /// Minimum Jaccard similarity to accept a match.
    min_confidence: f64,
    /// Length of character n-grams to generate.
    ngram_size: usize,
}

impl EmbeddingStore {
    /// Create a new store with the given minimum similarity threshold.
    ///
    /// A typical default is 0.65; higher values require closer matches.
    pub fn new(min_confidence: f64) -> Self {
        Self {
            entries: Vec::new(),
            min_confidence,
            ngram_size: 3,
        }
    }

    /// Add a labeled example to the store.
    pub fn insert(
        &mut self,
        description: &str,
        category: &str,
        subcategory: &str,
        confidence: f64,
    ) {
        let normalized = Self::normalize(description);
        let ngrams = Self::compute_ngrams(&normalized, self.ngram_size);
        self.entries.push(EmbeddingEntry {
            description: normalized,
            ngrams,
            category: category.to_string(),
            subcategory: subcategory.to_string(),
            confidence,
        });
    }

    /// Find the best match for the given description using character n-gram
    /// Jaccard similarity.
    ///
    /// Returns `Some((category, subcategory, similarity))` if the best match
    /// exceeds `min_confidence`, otherwise `None`.
    pub fn find_similar(&self, description: &str) -> Option<(String, String, f64)> {
        if self.entries.is_empty() {
            return None;
        }

        let normalized = Self::normalize(description);
        let query_ngrams = Self::compute_ngrams(&normalized, self.ngram_size);

        if query_ngrams.is_empty() {
            return None;
        }

        let mut best_similarity: f64 = 0.0;
        let mut best_idx: Option<usize> = None;

        for (i, entry) in self.entries.iter().enumerate() {
            let sim = Self::jaccard_similarity(&query_ngrams, &entry.ngrams);
            if sim > best_similarity {
                best_similarity = sim;
                best_idx = Some(i);
            }
        }

        match best_idx {
            Some(idx) if best_similarity >= self.min_confidence => {
                let entry = &self.entries[idx];
                Some((
                    entry.category.clone(),
                    entry.subcategory.clone(),
                    best_similarity,
                ))
            }
            _ => None,
        }
    }

    /// Number of entries in the store.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the store has no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    // ── Private helpers ──

    /// Normalize a description: lowercase, remove digits and punctuation,
    /// collapse whitespace.
    fn normalize(description: &str) -> String {
        description
            .to_lowercase()
            .chars()
            .map(|c| {
                if c.is_ascii_alphabetic() || c == ' ' {
                    c
                } else {
                    ' '
                }
            })
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Compute character n-grams of the given size from a normalized string.
    fn compute_ngrams(text: &str, n: usize) -> HashSet<String> {
        if text.len() < n {
            let mut set = HashSet::new();
            if !text.is_empty() {
                set.insert(text.to_string());
            }
            return set;
        }
        text.as_bytes()
            .windows(n)
            .map(|w| String::from_utf8_lossy(w).into_owned())
            .collect()
    }

    /// Jaccard similarity: |A intersection B| / |A union B|.
    fn jaccard_similarity(a: &HashSet<String>, b: &HashSet<String>) -> f64 {
        if a.is_empty() && b.is_empty() {
            return 0.0;
        }
        let intersection = a.intersection(b).count();
        let union = a.union(b).count();
        if union == 0 {
            return 0.0;
        }
        intersection as f64 / union as f64
    }
}

impl SemanticCategorizer for EmbeddingStore {
    fn categorize(&self, txn: &UncategorizedTransaction) -> Option<CategoryAssignment> {
        self.find_similar(&txn.description)
            .map(|(cat, sub, sim)| CategoryAssignment {
                transaction_id: txn.id,
                category: cat,
                subcategory: sub,
                merchant_name: String::new(),
                confidence: sim,
                source_tier: CategorizationTier::SemanticSearch,
            })
    }

    fn learn(&mut self, description: &str, category: &str, subcategory: &str, confidence: f64) {
        self.insert(description, category, subcategory, confidence);
    }

    fn index_size(&self) -> usize {
        self.len()
    }
}

impl SemanticVectorIngest for EmbeddingStore {
    /// Jaccard is a lexical backend — it ignores `_vector` completely and
    /// simply inserts the description. Returns `true` as long as the
    /// description survives normalization (non-empty after lowercasing /
    /// stripping non-letters).
    fn learn_with_vector(
        &mut self,
        description: &str,
        category: &str,
        subcategory: &str,
        confidence: f64,
        _vector: Option<&[f32]>,
    ) -> bool {
        if description.trim().is_empty() {
            return false;
        }
        self.insert(description, category, subcategory, confidence);
        true
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use rust_decimal::Decimal;
    use uuid::Uuid;

    use super::*;

    #[test]
    fn insert_and_len() {
        let mut store = EmbeddingStore::new(0.65);
        assert_eq!(store.len(), 0);
        assert!(store.is_empty());

        store.insert("STARBUCKS COFFEE", "food_dining", "coffee_shops", 0.95);
        assert_eq!(store.len(), 1);
        assert!(!store.is_empty());

        store.insert("SHELL GAS STATION", "transportation", "gas_fuel", 0.90);
        assert_eq!(store.len(), 2);
    }

    #[test]
    fn find_similar_exact_match() {
        let mut store = EmbeddingStore::new(0.5);
        store.insert("STARBUCKS COFFEE", "food_dining", "coffee_shops", 0.95);

        // Same description should match with similarity ~1.0
        let result = store.find_similar("STARBUCKS COFFEE");
        assert!(result.is_some());
        let (cat, sub, sim) = result.unwrap();
        assert_eq!(cat, "food_dining");
        assert_eq!(sub, "coffee_shops");
        assert!(
            sim > 0.99,
            "exact match similarity should be ~1.0, got {sim}"
        );
    }

    #[test]
    fn find_similar_close_match() {
        let mut store = EmbeddingStore::new(0.5);
        store.insert(
            "STARBUCKS COFFEE #1234",
            "food_dining",
            "coffee_shops",
            0.95,
        );

        // Similar but not identical (different store number)
        let result = store.find_similar("STARBUCKS COFFEE #5678");
        assert!(result.is_some());
        let (cat, sub, sim) = result.unwrap();
        assert_eq!(cat, "food_dining");
        assert_eq!(sub, "coffee_shops");
        assert!(
            sim > 0.5,
            "similar descriptions should have reasonable similarity, got {sim}"
        );
    }

    #[test]
    fn find_similar_no_match_below_threshold() {
        let mut store = EmbeddingStore::new(0.65);
        store.insert("STARBUCKS COFFEE", "food_dining", "coffee_shops", 0.95);

        // Completely different description
        let result = store.find_similar("UNITED AIRLINES FLIGHT");
        assert!(
            result.is_none(),
            "very different descriptions should not match"
        );
    }

    #[test]
    fn find_similar_empty_store() {
        let store = EmbeddingStore::new(0.65);
        let result = store.find_similar("ANYTHING");
        assert!(result.is_none());
    }

    #[test]
    fn find_similar_picks_best_match() {
        let mut store = EmbeddingStore::new(0.3);
        store.insert("SHELL GAS STATION", "transportation", "gas_fuel", 0.90);
        store.insert("STARBUCKS COFFEE SHOP", "food_dining", "coffee_shops", 0.95);
        store.insert("STARBUCKS RESERVE", "food_dining", "coffee_shops", 0.95);

        let result = store.find_similar("STARBUCKS COFFEE");
        assert!(result.is_some());
        let (cat, _sub, _sim) = result.unwrap();
        assert_eq!(cat, "food_dining");
    }

    #[test]
    fn jaccard_similarity_known_sets() {
        let a: HashSet<String> = ["abc", "bcd", "cde", "def"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let b: HashSet<String> = ["abc", "bcd", "xyz"]
            .iter()
            .map(|s| s.to_string())
            .collect();

        let sim = EmbeddingStore::jaccard_similarity(&a, &b);
        // intersection = {abc, bcd} = 2
        // union = {abc, bcd, cde, def, xyz} = 5
        // Jaccard = 2/5 = 0.4
        assert!((sim - 0.4).abs() < 1e-10, "expected 0.4, got {sim}");
    }

    #[test]
    fn jaccard_similarity_identical_sets() {
        let a: HashSet<String> = ["abc", "bcd"].iter().map(|s| s.to_string()).collect();
        let sim = EmbeddingStore::jaccard_similarity(&a, &a);
        assert!((sim - 1.0).abs() < 1e-10);
    }

    #[test]
    fn jaccard_similarity_disjoint_sets() {
        let a: HashSet<String> = ["abc", "bcd"].iter().map(|s| s.to_string()).collect();
        let b: HashSet<String> = ["xyz", "yzz"].iter().map(|s| s.to_string()).collect();
        let sim = EmbeddingStore::jaccard_similarity(&a, &b);
        assert!((sim - 0.0).abs() < 1e-10);
    }

    #[test]
    fn normalization_strips_digits_and_punctuation() {
        let normalized = EmbeddingStore::normalize("STARBUCKS #1234 - Coffee!");
        assert_eq!(normalized, "starbucks coffee");
    }

    #[test]
    fn normalization_collapses_whitespace() {
        let normalized = EmbeddingStore::normalize("  SHELL   GAS   STATION  ");
        assert_eq!(normalized, "shell gas station");
    }

    #[test]
    fn semantic_categorizer_trait_categorize() {
        let mut store = EmbeddingStore::new(0.5);
        store.insert("STARBUCKS COFFEE", "food_dining", "coffee_shops", 0.95);

        let txn = UncategorizedTransaction {
            id: Uuid::new_v4(),
            description: "STARBUCKS COFFEE #9999".to_string(),
            amount: Decimal::new(-575, 2),
            date: NaiveDate::from_ymd_opt(2026, 1, 15).unwrap(),
            mcc: None,
        };

        let result = store.categorize(&txn);
        assert!(result.is_some());
        let assignment = result.unwrap();
        assert_eq!(assignment.transaction_id, txn.id);
        assert_eq!(assignment.category, "food_dining");
        assert_eq!(assignment.subcategory, "coffee_shops");
        assert_eq!(assignment.source_tier, CategorizationTier::SemanticSearch);
        assert!(assignment.confidence > 0.5);
    }

    #[test]
    fn semantic_categorizer_trait_learn_and_index_size() {
        let mut store = EmbeddingStore::new(0.65);
        assert_eq!(store.index_size(), 0);

        store.learn(
            "NETFLIX SUBSCRIPTION",
            "entertainment",
            "streaming_services",
            0.95,
        );
        assert_eq!(store.index_size(), 1);

        store.learn(
            "HULU SUBSCRIPTION",
            "entertainment",
            "streaming_services",
            0.90,
        );
        assert_eq!(store.index_size(), 2);
    }
}
