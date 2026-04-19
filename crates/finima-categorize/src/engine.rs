use std::sync::{Arc, OnceLock, RwLock};
use std::time::Instant;

use regex::Regex;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::tier0::MerchantRegistry;
use crate::tier1::PatternEngine;
use crate::tier2::SemanticCategorizer;
use crate::types::{
    CascadeResult, CategorizationTier, CategoryAssignment, TierStats, UncategorizedTransaction,
};

/// Errors raised while constructing a Tier 2 semantic backend from
/// [`crate::config::Tier2Config`]. The Jaccard backend is infallible, so
/// today this only surfaces RuVector construction failures (HNSW
/// allocation errors, etc.).
#[derive(Debug, thiserror::Error)]
pub enum SemanticBuildError {
    #[error("RuVector Tier 2 backend failed to construct: {0}")]
    RuVector(String),
}

/// Transaction-outcome prefixes (NSF, RETURNED, REVERSED, etc.) are bank
/// status markers, not merchants. When they appear at the start of a
/// description, they dominate merchant signals that follow — e.g.
/// "NSF - Optum - Payment" is an NSF fee, not an Optum payment.
///
/// This pass runs before Tier 0 merchant lookup so those markers short-circuit
/// to the correct category instead of being out-voted by a real merchant
/// alias further along in the string.
struct OutcomePrefixRule {
    re: Regex,
    category: &'static str,
    subcategory: &'static str,
    confidence: f64,
}

fn outcome_prefix_rules() -> &'static [OutcomePrefixRule] {
    static RULES: OnceLock<Vec<OutcomePrefixRule>> = OnceLock::new();
    RULES.get_or_init(|| {
        vec![
            OutcomePrefixRule {
                re: Regex::new(r"(?i)^\s*nsf\b").unwrap(),
                category: "fees_charges",
                subcategory: "overdraft_fees",
                confidence: 0.95,
            },
            OutcomePrefixRule {
                re: Regex::new(r"(?i)^\s*(returned|return)\s+(item|ach|check|deposit|payment)\b")
                    .unwrap(),
                category: "fees_charges",
                subcategory: "returned_item_fees",
                confidence: 0.92,
            },
            OutcomePrefixRule {
                re: Regex::new(r"(?i)^\s*(reversal|reversed)\b").unwrap(),
                category: "fees_charges",
                subcategory: "reversal",
                confidence: 0.90,
            },
        ]
    })
}

/// Match a transaction-outcome prefix (NSF, RETURNED, REVERSAL) at the start
/// of a description. Returns a categorized assignment (with a nil transaction
/// id — caller must populate) when the leading token is a bank status marker
/// rather than a merchant. Intended to run **before** Tier 0 merchant lookup
/// so these markers dominate any merchant aliases later in the description
/// (e.g. "NSF - Optum - Payment" is an NSF fee, not an Optum payment).
pub fn match_outcome_prefix(description: &str) -> Option<CategoryAssignment> {
    for rule in outcome_prefix_rules() {
        if rule.re.is_match(description) {
            return Some(CategoryAssignment {
                transaction_id: Uuid::nil(),
                category: rule.category.to_string(),
                subcategory: rule.subcategory.to_string(),
                merchant_name: String::new(),
                confidence: rule.confidence,
                source_tier: CategorizationTier::PatternEngine,
            });
        }
    }
    None
}

/// Run the synchronous tiers (outcome-prefix → Tier 0 merchant lookup →
/// Tier 1 pattern engine) for a single transaction description.
///
/// Returns the first matching assignment, or `None` if nothing in tiers 0-1
/// matches. The `transaction_id` on the returned assignment is left as
/// `Uuid::nil()` — the caller is responsible for populating it.
///
/// This is the single source of truth for the synchronous cascade. Both
/// [`CascadeEngine::categorize`] and the API categorization pipeline call
/// into it, so categorization rules only need to be maintained in one place.
pub fn cascade_tiers_0_1(
    registry: &MerchantRegistry,
    pattern_engine: &PatternEngine,
    description: &str,
    amount: Decimal,
    mcc: Option<u16>,
) -> Option<CategoryAssignment> {
    // Pre-Tier 0: outcome-prefix markers (NSF, RETURNED, REVERSAL) must beat
    // any merchant aliases later in the description.
    if let Some(a) = match_outcome_prefix(description) {
        return Some(a);
    }

    // Tier 0: merchant lookup
    if let Some(a) = registry.lookup(description, mcc) {
        return Some(a);
    }

    // Tier 1: pattern engine
    pattern_engine.match_pattern(description, amount)
}

/// The cascade categorization engine.
///
/// Runs transactions through tiers in order:
/// 1. Tier 0 - Merchant lookup (exact, fuzzy, MCC)
/// 2. Tier 1 - Pattern engine (regex + amount heuristics)
/// 3. Tier 2 - Semantic search (character n-gram similarity)
///
/// Tier 3 (LLM batch inference) will be added in a later phase.
pub struct CascadeEngine {
    merchant_registry: MerchantRegistry,
    pattern_engine: PatternEngine,
    semantic: Option<Arc<RwLock<dyn SemanticCategorizer>>>,
}

impl CascadeEngine {
    /// Create a new cascade engine with the given tiers.
    pub fn new(merchant_registry: MerchantRegistry, pattern_engine: PatternEngine) -> Self {
        Self {
            merchant_registry,
            pattern_engine,
            semantic: None,
        }
    }

    /// Builder method to attach a Tier 2 semantic categorizer.
    pub fn with_semantic(mut self, semantic: Arc<RwLock<dyn SemanticCategorizer>>) -> Self {
        self.semantic = Some(semantic);
        self
    }

    /// Build the default Tier 2 semantic store from configuration.
    ///
    /// Branches on [`Tier2Config::resolved_backend`]:
    ///   * [`Tier2Backend::Jaccard`] → [`EmbeddingStore::new`] (always
    ///     available).
    ///   * [`Tier2Backend::RuVector`] → [`RuVectorEmbeddingStore::new`] if
    ///     the `sona` feature is compiled in. Falls back to
    ///     [`EmbeddingStore`] otherwise (resolved_backend already warns).
    ///
    /// Returns an `Arc<RwLock<dyn SemanticCategorizer>>` ready to pass into
    /// [`CascadeEngine::with_semantic`]. Callers in `finima-api` should
    /// prefer this over hand-rolling the backend branch themselves so the
    /// Tier 2 construction policy lives in one place.
    pub fn build_semantic_from_config(
        cfg: &crate::config::Tier2Config,
        min_confidence: f64,
    ) -> Result<Arc<RwLock<dyn SemanticCategorizer>>, SemanticBuildError> {
        use crate::config::Tier2Backend;
        use crate::tier2::EmbeddingStore;

        match cfg.resolved_backend() {
            Tier2Backend::Jaccard => {
                let store = EmbeddingStore::new(min_confidence);
                Ok(Arc::new(RwLock::new(store)))
            }
            #[cfg(feature = "sona")]
            Tier2Backend::RuVector => {
                use crate::tier2::RuVectorEmbeddingStore;
                let store = RuVectorEmbeddingStore::new(cfg.clone(), min_confidence)
                    .map_err(|e| SemanticBuildError::RuVector(e.to_string()))?;
                Ok(Arc::new(RwLock::new(store)))
            }
            // Without the `sona` feature, `resolved_backend()` has already
            // downgraded `RuVector` → `Jaccard` (with a warning). This arm
            // is therefore unreachable at runtime but is required for
            // match exhaustiveness because the enum variant still exists.
            #[cfg(not(feature = "sona"))]
            Tier2Backend::RuVector => unreachable!(
                "resolved_backend() must downgrade RuVector to Jaccard without the `sona` feature"
            ),
        }
    }

    /// Convenience that chains [`Self::build_semantic_from_config`] into
    /// [`Self::with_semantic`].
    pub fn with_semantic_from_config(
        self,
        cfg: &crate::config::Tier2Config,
        min_confidence: f64,
    ) -> Result<Self, SemanticBuildError> {
        let store = Self::build_semantic_from_config(cfg, min_confidence)?;
        Ok(self.with_semantic(store))
    }

    /// Categorize a batch of transactions through the cascade.
    ///
    /// Returns assignments for categorized transactions and the IDs of
    /// those that could not be categorized by any available tier.
    pub fn categorize(&self, transactions: &[UncategorizedTransaction]) -> CascadeResult {
        let start = Instant::now();
        let total = transactions.len();

        let mut assignments: Vec<CategoryAssignment> = Vec::new();
        let mut remaining_txns: Vec<&UncategorizedTransaction> = transactions.iter().collect();
        let mut stats = TierStats {
            total,
            ..Default::default()
        };

        // ── Tiers 0-1: outcome-prefix + merchant lookup + pattern engine ──
        // Delegates to `cascade_tiers_0_1` so the API pipeline and the engine
        // share a single implementation.
        let mut still_remaining = Vec::new();
        for txn in &remaining_txns {
            if let Some(mut assignment) = cascade_tiers_0_1(
                &self.merchant_registry,
                &self.pattern_engine,
                &txn.description,
                txn.amount,
                txn.mcc,
            ) {
                assignment.transaction_id = txn.id;
                match assignment.source_tier {
                    CategorizationTier::MerchantLookup => stats.tier0_matched += 1,
                    CategorizationTier::PatternEngine => stats.tier1_matched += 1,
                    _ => {}
                }
                assignments.push(assignment);
            } else {
                still_remaining.push(*txn);
            }
        }
        remaining_txns = still_remaining;

        tracing::debug!(
            tier0 = stats.tier0_matched,
            tier1 = stats.tier1_matched,
            remaining = remaining_txns.len(),
            "tiers 0-1 (merchant + pattern) complete"
        );

        // ── Tier 2: Semantic Search ──
        if let Some(ref semantic) = self.semantic {
            let sem = semantic.read().expect("semantic categorizer lock poisoned");
            let mut still_remaining = Vec::new();
            for txn in &remaining_txns {
                if let Some(assignment) = sem.categorize(txn) {
                    assignments.push(assignment);
                    stats.tier2_matched += 1;
                } else {
                    still_remaining.push(*txn);
                }
            }
            remaining_txns = still_remaining;

            tracing::debug!(
                matched = stats.tier2_matched,
                remaining = remaining_txns.len(),
                "tier 2 (semantic search) complete"
            );
        }

        // ── Future: Tier 3 (LLM) goes here ──

        let remaining_ids: Vec<Uuid> = remaining_txns.iter().map(|t| t.id).collect();
        stats.elapsed_ms = start.elapsed().as_millis() as u64;

        tracing::info!(
            total,
            tier0 = stats.tier0_matched,
            tier1 = stats.tier1_matched,
            tier2 = stats.tier2_matched,
            remaining = remaining_ids.len(),
            elapsed_ms = stats.elapsed_ms,
            "cascade categorization complete"
        );

        CascadeResult {
            assignments,
            remaining: remaining_ids,
            stats,
        }
    }

    /// Access the merchant registry (e.g., to add learned merchants).
    pub fn merchant_registry(&self) -> &MerchantRegistry {
        &self.merchant_registry
    }

    /// Mutable access to the merchant registry (e.g., for learning).
    pub fn merchant_registry_mut(&mut self) -> &mut MerchantRegistry {
        &mut self.merchant_registry
    }
}

#[cfg(test)]
mod tests {
    use chrono::{NaiveDate, Utc};
    use rust_decimal::Decimal;
    use uuid::Uuid;

    use super::*;
    use crate::types::{MerchantEntry, MerchantSource};

    fn make_txn(desc: &str, amount: i64, mcc: Option<u16>) -> UncategorizedTransaction {
        UncategorizedTransaction {
            id: Uuid::new_v4(),
            description: desc.to_string(),
            amount: Decimal::new(amount, 2),
            date: NaiveDate::from_ymd_opt(2026, 1, 15).unwrap(),
            mcc,
        }
    }

    fn build_engine() -> CascadeEngine {
        let mut registry = MerchantRegistry::with_defaults();
        registry.add_merchant(MerchantEntry {
            canonical_name: "Starbucks".to_string(),
            aliases: vec!["SBUX".to_string(), "STARBUCKS COFFEE".to_string()],
            category: "food_dining".to_string(),
            subcategory: "coffee_shops".to_string(),
            confidence: 0.95,
            source: MerchantSource::SeedData,
            last_seen: Utc::now(),
        });
        registry.add_merchant(MerchantEntry {
            canonical_name: "Shell".to_string(),
            aliases: vec!["SHELL OIL".to_string()],
            category: "transportation".to_string(),
            subcategory: "gas_fuel".to_string(),
            confidence: 0.95,
            source: MerchantSource::SeedData,
            last_seen: Utc::now(),
        });

        let pattern_engine = PatternEngine::with_defaults();
        CascadeEngine::new(registry, pattern_engine)
    }

    #[test]
    fn tier0_matches_known_merchant() {
        let engine = build_engine();
        let txns = vec![make_txn("STARBUCKS #1234", -575, None)];
        let result = engine.categorize(&txns);

        assert_eq!(result.assignments.len(), 1);
        assert_eq!(result.remaining.len(), 0);
        assert_eq!(result.stats.tier0_matched, 1);
        assert_eq!(result.assignments[0].category, "food_dining");
    }

    #[test]
    fn tier1_catches_pattern_after_tier0_miss() {
        let engine = build_engine();
        let txns = vec![make_txn("NETFLIX.COM", -1599, None)];
        let result = engine.categorize(&txns);

        assert_eq!(result.assignments.len(), 1);
        assert_eq!(result.remaining.len(), 0);
        assert_eq!(result.stats.tier0_matched, 0);
        assert_eq!(result.stats.tier1_matched, 1);
        assert_eq!(result.assignments[0].category, "entertainment");
    }

    #[test]
    fn unmatched_goes_to_remaining() {
        let engine = build_engine();
        let txns = vec![make_txn("OBSCURE PLACE 999", -4200, None)];
        let result = engine.categorize(&txns);

        assert_eq!(result.assignments.len(), 0);
        assert_eq!(result.remaining.len(), 1);
    }

    #[test]
    fn mixed_batch_distributes_across_tiers() {
        let engine = build_engine();
        let txns = vec![
            make_txn("STARBUCKS #5678", -650, None),
            make_txn("SHELL OIL 0042", -4500, None),
            make_txn("NETFLIX.COM", -1599, None),
            make_txn("PAYROLL ACME INC", 350000, None),
            make_txn("RANDOM WEIRD THING", -100, None),
        ];
        let result = engine.categorize(&txns);

        assert_eq!(result.stats.tier0_matched, 2); // Starbucks + Shell
        assert_eq!(result.stats.tier1_matched, 2); // Netflix + Payroll
        assert_eq!(result.remaining.len(), 1); // Random
        assert_eq!(result.stats.total, 5);
    }

    #[test]
    fn mcc_fallback_in_tier0() {
        let mut registry = MerchantRegistry::with_defaults();
        registry.mcc_map_insert(5411, "food_dining", "groceries");

        let engine = CascadeEngine::new(registry, PatternEngine::with_defaults());
        let txns = vec![make_txn("UNKNOWN STORE 1234", -8799, Some(5411))];
        let result = engine.categorize(&txns);

        assert_eq!(result.assignments.len(), 1);
        assert_eq!(result.stats.tier0_matched, 1);
        assert_eq!(result.assignments[0].category, "food_dining");
    }

    #[test]
    fn tier2_catches_semantic_match_after_tier1_miss() {
        use crate::tier2::EmbeddingStore;

        let mut store = EmbeddingStore::new(0.5);
        // Teach the store about a merchant that Tier 0 and Tier 1 don't know
        store.insert("JOES ARTISAN BAKERY", "food_dining", "bakeries", 0.90);

        let semantic = Arc::new(RwLock::new(store));
        let engine = build_engine().with_semantic(semantic);

        let txns = vec![make_txn("JOES ARTISAN BAKERY #42", -1250, None)];
        let result = engine.categorize(&txns);

        assert_eq!(result.assignments.len(), 1);
        assert_eq!(result.remaining.len(), 0);
        assert_eq!(result.stats.tier0_matched, 0);
        assert_eq!(result.stats.tier1_matched, 0);
        assert_eq!(result.stats.tier2_matched, 1);
        assert_eq!(result.assignments[0].category, "food_dining");
        assert_eq!(result.assignments[0].subcategory, "bakeries");
        assert_eq!(
            result.assignments[0].source_tier,
            crate::types::CategorizationTier::SemanticSearch
        );
    }

    #[test]
    fn cascade_all_three_tiers() {
        use crate::tier2::EmbeddingStore;

        let mut store = EmbeddingStore::new(0.5);
        store.insert("JOES ARTISAN BAKERY", "food_dining", "bakeries", 0.90);

        let semantic = Arc::new(RwLock::new(store));
        let engine = build_engine().with_semantic(semantic);

        let txns = vec![
            // Tier 0: known merchant
            make_txn("STARBUCKS #1234", -575, None),
            // Tier 1: regex pattern
            make_txn("NETFLIX.COM", -1599, None),
            // Tier 2: semantic match
            make_txn("JOES ARTISAN BAKERY DOWNTOWN", -2400, None),
            // No match: goes to remaining
            make_txn("XYZZY UNKNOWN THING", -100, None),
        ];
        let result = engine.categorize(&txns);

        assert_eq!(result.stats.tier0_matched, 1);
        assert_eq!(result.stats.tier1_matched, 1);
        assert_eq!(result.stats.tier2_matched, 1);
        assert_eq!(result.remaining.len(), 1);
        assert_eq!(result.assignments.len(), 3);
        assert_eq!(result.stats.total, 4);
    }

    #[test]
    fn nsf_prefix_beats_merchant_alias_in_rest_of_description() {
        // Regression: "NSF - Optum - Payment" was being categorized as
        // debt_payment/personal_loan because "optum" won the Tier 0 substring
        // tiebreaker over "nsf". The pre-Tier-0 outcome-prefix pass should
        // short-circuit to fees_charges/overdraft_fees.
        let mut registry = MerchantRegistry::with_defaults();
        registry.add_merchant(MerchantEntry {
            canonical_name: "Optum".to_string(),
            aliases: vec!["OPTUM".to_string(), "OPTUM PAYMENT".to_string()],
            category: "debt_payment".to_string(),
            subcategory: "personal_loan".to_string(),
            confidence: 0.95,
            source: MerchantSource::SeedData,
            last_seen: Utc::now(),
        });
        registry.add_merchant(MerchantEntry {
            canonical_name: "Insufficient Funds".to_string(),
            aliases: vec!["NSF".to_string(), "OVERDRAFT".to_string()],
            category: "fees_charges".to_string(),
            subcategory: "overdraft_fees".to_string(),
            confidence: 0.95,
            source: MerchantSource::SeedData,
            last_seen: Utc::now(),
        });

        let engine = CascadeEngine::new(registry, PatternEngine::with_defaults());
        let txns = vec![make_txn("NSF - Optum - Payment", -1000, None)];
        let result = engine.categorize(&txns);

        assert_eq!(result.assignments.len(), 1);
        let a = &result.assignments[0];
        assert_eq!(a.category, "fees_charges");
        assert_eq!(a.subcategory, "overdraft_fees");
        assert_eq!(
            a.source_tier,
            crate::types::CategorizationTier::PatternEngine
        );
    }

    #[test]
    fn outcome_prefix_does_not_fire_mid_description() {
        // "NSF" appearing mid-string (e.g. as part of a merchant name) should
        // NOT trigger the outcome-prefix pass — only leading-position markers.
        let mut registry = MerchantRegistry::with_defaults();
        registry.add_merchant(MerchantEntry {
            canonical_name: "Transfer NSF Corp".to_string(),
            aliases: vec!["TRANSFER NSF CORP".to_string()],
            category: "transportation".to_string(),
            subcategory: "rideshare_taxi".to_string(),
            confidence: 0.95,
            source: MerchantSource::SeedData,
            last_seen: Utc::now(),
        });
        let engine = CascadeEngine::new(registry, PatternEngine::with_defaults());
        let txns = vec![make_txn("TRANSFER NSF CORP", -500, None)];
        let result = engine.categorize(&txns);

        assert_eq!(result.assignments.len(), 1);
        // Merchant wins, not fees_charges
        assert_eq!(result.assignments[0].category, "transportation");
    }

    #[test]
    fn ui_benefit_beats_external_deposit_prefix() {
        // Regression: "External Deposit - WA ST EMPLOY SEC - UI BENEFIT"
        // was being categorized as transfer/ach_transfer because both the
        // "External Deposit" and "WA ST EMPLOY SEC" seed aliases had confidence
        // 0.4 and the HashMap iteration order picked "External Deposit" first.
        // With UI Benefit at default 0.95, the real payee must win.
        let mut registry = MerchantRegistry::with_defaults();
        registry.add_merchant(MerchantEntry {
            canonical_name: "External Deposit".to_string(),
            aliases: vec!["EXTERNAL DEPOSIT".to_string()],
            category: "transfer".to_string(),
            subcategory: "ach_transfer".to_string(),
            confidence: 0.40,
            source: MerchantSource::SeedData,
            last_seen: Utc::now(),
        });
        registry.add_merchant(MerchantEntry {
            canonical_name: "UI Benefit".to_string(),
            aliases: vec![
                "UI BENEFIT".to_string(),
                "WA ST EMPLOY SEC".to_string(),
                "UNEMPLOYMENT BENEFIT".to_string(),
            ],
            category: "income".to_string(),
            subcategory: "government_benefits".to_string(),
            confidence: 0.95,
            source: MerchantSource::SeedData,
            last_seen: Utc::now(),
        });

        let engine = CascadeEngine::new(registry, PatternEngine::with_defaults());
        let txns = vec![make_txn(
            "External Deposit - WA ST EMPLOY SEC - UI BENEFIT",
            103700,
            None,
        )];
        let result = engine.categorize(&txns);

        assert_eq!(result.assignments.len(), 1);
        let a = &result.assignments[0];
        assert_eq!(a.category, "income");
        assert_eq!(a.subcategory, "government_benefits");
    }

    #[test]
    fn build_semantic_from_config_jaccard_backend() {
        use crate::config::{Tier2Backend, Tier2Config};

        let cfg = Tier2Config {
            backend: Tier2Backend::Jaccard,
            ..Tier2Config::default()
        };
        let store = CascadeEngine::build_semantic_from_config(&cfg, 0.65).expect("jaccard build");
        {
            let guard = store.read().expect("lock");
            assert_eq!(guard.index_size(), 0);
        }
        {
            let mut guard = store.write().expect("lock");
            guard.learn("JOES ARTISAN BAKERY", "food_dining", "bakeries", 0.90);
            assert_eq!(guard.index_size(), 1);
        }
    }

    #[test]
    fn with_semantic_from_config_chains() {
        use crate::config::{Tier2Backend, Tier2Config};

        let cfg = Tier2Config {
            backend: Tier2Backend::Jaccard,
            ..Tier2Config::default()
        };
        let engine = build_engine()
            .with_semantic_from_config(&cfg, 0.65)
            .expect("chain");

        // Sanity: engine still cascades normally with an (empty) semantic
        // store attached.
        let txns = vec![make_txn("STARBUCKS #1234", -575, None)];
        let result = engine.categorize(&txns);
        assert_eq!(result.stats.tier0_matched, 1);
        assert_eq!(result.stats.tier2_matched, 0);
        assert_eq!(result.remaining.len(), 0);
    }

    #[cfg(feature = "sona")]
    #[test]
    fn build_semantic_from_config_ruvector_backend() {
        use crate::config::{Tier2Backend, Tier2Config};

        let cfg = Tier2Config {
            backend: Tier2Backend::RuVector,
            dim: 64,
            ..Tier2Config::default()
        };
        let store = CascadeEngine::build_semantic_from_config(&cfg, 0.65).expect("ruvector build");
        let guard = store.read().expect("lock");
        assert_eq!(guard.index_size(), 0);
    }

    #[test]
    fn cascade_without_semantic_still_works() {
        // Ensure the engine works fine when no semantic categorizer is attached
        let engine = build_engine();
        let txns = vec![
            make_txn("STARBUCKS #1234", -575, None),
            make_txn("OBSCURE PLACE 999", -4200, None),
        ];
        let result = engine.categorize(&txns);

        assert_eq!(result.stats.tier0_matched, 1);
        assert_eq!(result.stats.tier2_matched, 0);
        assert_eq!(result.remaining.len(), 1);
    }
}
