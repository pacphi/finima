use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The result of categorizing a single transaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryAssignment {
    pub transaction_id: Uuid,
    pub category: String,
    pub subcategory: String,
    pub merchant_name: String,
    pub confidence: f64,
    pub source_tier: CategorizationTier,
}

/// Which tier in the cascade produced the categorization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CategorizationTier {
    MerchantLookup,
    PatternEngine,
    SemanticSearch,
    LlmInference,
    UserOverride,
}

impl CategorizationTier {
    /// Returns the database-friendly string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::MerchantLookup => "merchant_lookup",
            Self::PatternEngine => "pattern_engine",
            Self::SemanticSearch => "semantic_search",
            Self::LlmInference => "llm",
            Self::UserOverride => "user",
        }
    }
}

impl std::fmt::Display for CategorizationTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Aggregate statistics for a cascade run.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TierStats {
    pub tier0_matched: usize,
    pub tier1_matched: usize,
    pub tier2_matched: usize,
    pub tier3_matched: usize,
    pub tier3_failed: usize,
    pub total: usize,
    pub elapsed_ms: u64,
}

/// A known merchant with its category mapping.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerchantEntry {
    pub canonical_name: String,
    pub aliases: Vec<String>,
    pub category: String,
    pub subcategory: String,
    pub confidence: f64,
    pub source: MerchantSource,
    #[serde(default = "Utc::now")]
    pub last_seen: DateTime<Utc>,
}

/// How a merchant entry was created.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MerchantSource {
    MccDatabase,
    SeedData,
    LlmLearned,
    UserDefined,
}

impl MerchantSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::MccDatabase => "mcc",
            Self::SeedData => "seed",
            Self::LlmLearned => "llm_learned",
            Self::UserDefined => "user_defined",
        }
    }
}

/// A transaction that has not yet been categorized.
#[derive(Debug, Clone)]
pub struct UncategorizedTransaction {
    pub id: Uuid,
    pub description: String,
    pub amount: Decimal,
    pub date: NaiveDate,
    pub mcc: Option<u16>,
}

/// The result of running the cascade engine on a batch.
#[derive(Debug, Clone)]
pub struct CascadeResult {
    pub assignments: Vec<CategoryAssignment>,
    pub remaining: Vec<Uuid>,
    pub stats: TierStats,
}
