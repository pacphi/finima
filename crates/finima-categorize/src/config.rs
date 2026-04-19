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
    /// Tier 2 backend selection and tuning.
    pub tier2: Tier2Config,
}

impl Default for CategorizeConfig {
    fn default() -> Self {
        Self {
            fuzzy_threshold: 0.88,
            pattern_min_confidence: 0.70,
            semantic_min_confidence: 0.85,
            prefix_length: 3,
            tier2: Tier2Config::default(),
        }
    }
}

/// Tier 2 (semantic similarity) backend selector.
///
/// See [`Tier2Config`] and `config/categorize.yaml` for how this is wired.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tier2Backend {
    /// Pure-Rust character n-gram Jaccard store (default).
    ///
    /// No extra dependencies; suitable for small portfolios and CI. Scores
    /// plateau quickly as the corpus grows beyond a few thousand examples.
    #[default]
    Jaccard,
    /// HNSW-backed `VectorDB` from `ruvector-core`, optionally paired with
    /// `ruvector-sona` ReasoningBank patterns (ADR-017). Requires the
    /// `sona` feature to be enabled at compile time. If the feature is off,
    /// [`Tier2Config::resolved_backend`] returns [`Tier2Backend::Jaccard`]
    /// and emits a warning.
    RuVector,
}

impl Tier2Backend {
    /// Parse a YAML-friendly string. Case-insensitive. Returns `None` for
    /// unrecognized values.
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "jaccard" => Some(Self::Jaccard),
            "ruvector" | "rv" | "hnsw" => Some(Self::RuVector),
            _ => None,
        }
    }

    /// YAML-friendly serialization (lowercase).
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Jaccard => "jaccard",
            Self::RuVector => "ruvector",
        }
    }
}

/// Tier 2 backend configuration.
///
/// Vector-store tunables (`dim`, `hnsw_*`) are only consulted when
/// [`Tier2Config::resolved_backend`] returns [`Tier2Backend::RuVector`].
/// Jaccard ignores them completely.
#[derive(Debug, Clone)]
pub struct Tier2Config {
    /// Requested backend. The *resolved* backend is obtained via
    /// [`Tier2Config::resolved_backend`], which may downgrade to
    /// [`Tier2Backend::Jaccard`] if the requested backend is unavailable
    /// in this build.
    pub backend: Tier2Backend,
    /// Embedding dimensionality expected by the vector store.
    /// Ignored by the Jaccard backend.
    pub dim: usize,
    /// HNSW `M` parameter — connections per layer. Higher = better recall
    /// at the cost of memory and build time.
    pub hnsw_m: usize,
    /// HNSW `ef_construction` — candidate list size during insertion.
    pub hnsw_ef_construction: usize,
    /// HNSW `ef_search` — candidate list size during queries. Can be
    /// overridden per-query via `ruvector_core::types::SearchQuery`.
    pub hnsw_ef_search: usize,
    /// If true, spawn a background bootstrap task on first engine
    /// construction that seeds the Tier 2 store from historical
    /// categorized transactions.
    pub bootstrap_on_start: bool,
    /// Cap on bootstrap corpus size. `0` means unbounded.
    pub bootstrap_max_examples: usize,
}

impl Default for Tier2Config {
    fn default() -> Self {
        Self {
            backend: Tier2Backend::Jaccard,
            dim: 384,
            hnsw_m: 32,
            hnsw_ef_construction: 200,
            hnsw_ef_search: 100,
            bootstrap_on_start: true,
            bootstrap_max_examples: 0,
        }
    }
}

impl Tier2Config {
    /// Resolve the *effective* backend, accounting for compile-time feature
    /// availability.
    ///
    /// If the caller requested [`Tier2Backend::RuVector`] but the `sona`
    /// feature is not enabled in this build, we silently fall back to
    /// [`Tier2Backend::Jaccard`] and emit a `tracing::warn!` event. Callers
    /// that want to surface this to the user should compare the returned
    /// value with [`Self::backend`].
    pub fn resolved_backend(&self) -> Tier2Backend {
        match self.backend {
            Tier2Backend::Jaccard => Tier2Backend::Jaccard,
            Tier2Backend::RuVector => {
                #[cfg(feature = "sona")]
                {
                    Tier2Backend::RuVector
                }
                #[cfg(not(feature = "sona"))]
                {
                    tracing::warn!(
                        "categorize.tier2.backend=ruvector requested but `sona` feature is not enabled; falling back to jaccard"
                    );
                    Tier2Backend::Jaccard
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tier2_backend_parse() {
        assert_eq!(Tier2Backend::parse("jaccard"), Some(Tier2Backend::Jaccard));
        assert_eq!(Tier2Backend::parse("JACCARD"), Some(Tier2Backend::Jaccard));
        assert_eq!(
            Tier2Backend::parse("ruvector"),
            Some(Tier2Backend::RuVector)
        );
        assert_eq!(Tier2Backend::parse("HNSW"), Some(Tier2Backend::RuVector));
        assert_eq!(Tier2Backend::parse("nope"), None);
    }

    #[test]
    fn resolved_backend_without_sona_feature_is_jaccard() {
        let cfg = Tier2Config {
            backend: Tier2Backend::RuVector,
            ..Default::default()
        };
        // Without the `sona` feature compiled in, resolution downgrades.
        // This test asserts behavior under the *current* build — so it
        // passes either way: when `sona` is on we keep RuVector, when
        // `sona` is off we get Jaccard.
        let resolved = cfg.resolved_backend();
        #[cfg(feature = "sona")]
        assert_eq!(resolved, Tier2Backend::RuVector);
        #[cfg(not(feature = "sona"))]
        assert_eq!(resolved, Tier2Backend::Jaccard);
    }

    #[test]
    fn defaults_sensible() {
        let cfg = CategorizeConfig::default();
        assert!(cfg.fuzzy_threshold > 0.0 && cfg.fuzzy_threshold < 1.0);
        assert_eq!(cfg.tier2.backend, Tier2Backend::Jaccard);
        assert_eq!(cfg.tier2.dim, 384);
    }
}
