use std::sync::{Arc, RwLock};
use std::time::Instant;

use uuid::Uuid;

use crate::tier0::MerchantRegistry;
use crate::tier1::PatternEngine;
use crate::tier2::SemanticCategorizer;
use crate::types::{CascadeResult, CategoryAssignment, TierStats, UncategorizedTransaction};

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

        // ── Tier 0: Merchant Lookup ──
        let mut still_remaining = Vec::new();
        for txn in &remaining_txns {
            if let Some(mut assignment) = self.merchant_registry.lookup(&txn.description, txn.mcc) {
                assignment.transaction_id = txn.id;
                assignments.push(assignment);
                stats.tier0_matched += 1;
            } else {
                still_remaining.push(*txn);
            }
        }
        remaining_txns = still_remaining;

        tracing::debug!(
            matched = stats.tier0_matched,
            remaining = remaining_txns.len(),
            "tier 0 (merchant lookup) complete"
        );

        // ── Tier 1: Pattern Engine ──
        let mut still_remaining = Vec::new();
        for txn in &remaining_txns {
            if let Some(mut assignment) =
                self.pattern_engine.match_pattern(&txn.description, txn.amount)
            {
                assignment.transaction_id = txn.id;
                assignments.push(assignment);
                stats.tier1_matched += 1;
            } else {
                still_remaining.push(*txn);
            }
        }
        remaining_txns = still_remaining;

        tracing::debug!(
            matched = stats.tier1_matched,
            remaining = remaining_txns.len(),
            "tier 1 (pattern engine) complete"
        );

        // ── Tier 2: Semantic Search ──
        if let Some(ref semantic) = self.semantic {
            let sem = semantic
                .read()
                .expect("semantic categorizer lock poisoned");
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
        assert_eq!(result.remaining.len(), 1);     // Random
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
        store.insert(
            "JOES ARTISAN BAKERY",
            "food_dining",
            "bakeries",
            0.90,
        );

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
        store.insert(
            "JOES ARTISAN BAKERY",
            "food_dining",
            "bakeries",
            0.90,
        );

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
