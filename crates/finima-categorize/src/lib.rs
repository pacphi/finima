//! Tiered transaction categorization engine for Finima.
//!
//! Implements a cascade of categorization tiers:
//! - **Tier 0**: Merchant lookup (exact match, fuzzy match, MCC codes)
//! - **Tier 1**: Pattern engine (regex rules + amount heuristics)
//! - **Tier 2**: Semantic search (planned, RuVector HNSW)
//! - **Tier 3**: LLM batch inference (planned)
//!
//! Transactions flow through tiers in order. Each tier handles the
//! subset that previous tiers could not categorize, so expensive
//! tiers (LLM) only process the long tail of ambiguous descriptions.

pub mod config;
pub mod engine;
pub mod tier0;
pub mod tier1;
pub mod tier2;
pub mod types;

pub use config::CategorizeConfig;
pub use engine::{cascade_tiers_0_1, match_outcome_prefix, CascadeEngine};
pub use tier0::MerchantRegistry;
pub use tier1::PatternEngine;
pub use tier2::{EmbeddingStore, SemanticCategorizer};
pub use types::*;

/// Embedded seed merchant data.
pub const SEED_MERCHANTS_JSON: &str = include_str!("../data/seed_merchants.json");
