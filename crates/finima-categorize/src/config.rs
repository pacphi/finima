/// Configuration for the categorization cascade.
#[derive(Debug, Clone)]
pub struct CategorizeConfig {
    /// Minimum Jaro-Winkler similarity for fuzzy merchant matching.
    pub fuzzy_threshold: f64,
    /// Minimum confidence to accept a Tier 1 pattern match.
    pub pattern_min_confidence: f64,
    /// Minimum confidence to accept a Tier 2 semantic match.
    pub semantic_min_confidence: f64,
    /// Number of prefix characters used for the fuzzy-match prefix index.
    pub prefix_length: usize,
}

impl Default for CategorizeConfig {
    fn default() -> Self {
        Self {
            fuzzy_threshold: 0.88,
            pattern_min_confidence: 0.70,
            semantic_min_confidence: 0.85,
            prefix_length: 3,
        }
    }
}
