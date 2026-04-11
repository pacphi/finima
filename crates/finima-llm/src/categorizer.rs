use std::sync::Arc;

use crate::client::{
    CategorizationBatch, CategorizationResult, LlmClient, OverridePattern, TransactionInput,
};
use crate::error::LlmError;

/// Summary report of a categorization run.
#[derive(Debug, Clone)]
pub struct CategorizationReport {
    pub results: Vec<CategorizationResult>,
    pub pattern_matched: usize,
    pub llm_categorized: usize,
    pub flagged: usize,
    pub failed: usize,
}

/// Orchestrates batch categorization of transactions.
///
/// First applies user override pattern matching (no LLM needed),
/// then sends remaining transactions to the LLM in batches.
pub struct Categorizer {
    llm_client: Arc<dyn LlmClient>,
    batch_size: usize,
}

impl Categorizer {
    pub fn new(llm_client: Arc<dyn LlmClient>) -> Self {
        Self {
            llm_client,
            batch_size: 20,
        }
    }

    pub fn with_batch_size(mut self, batch_size: usize) -> Self {
        self.batch_size = batch_size;
        self
    }

    /// Categorize transactions using pattern matching and LLM.
    ///
    /// 1. Apply override pattern matching (substring, case-insensitive).
    /// 2. Split remaining into batches.
    /// 3. Call LLM for each batch.
    /// 4. Collect and report results.
    pub async fn categorize_transactions(
        &self,
        transactions: Vec<TransactionInput>,
        overrides: Vec<OverridePattern>,
    ) -> Result<CategorizationReport, LlmError> {
        let mut all_results: Vec<CategorizationResult> = Vec::new();
        let mut pattern_matched: usize = 0;
        let mut llm_categorized: usize = 0;
        let mut flagged: usize = 0;
        let mut failed: usize = 0;

        let mut remaining: Vec<TransactionInput> = Vec::new();

        // Step 1: pattern matching
        for txn in &transactions {
            let desc_lower = txn.description.to_lowercase();
            let mut matched = false;

            for ov in &overrides {
                if desc_lower.contains(&ov.pattern.to_lowercase()) {
                    all_results.push(CategorizationResult {
                        transaction_id: txn.id,
                        category: ov.category.clone(),
                        subcategory: ov.subcategory.clone(),
                        merchant_name: crate::enricher::normalize_merchant(&txn.description),
                        confidence: 1.0,
                    });
                    pattern_matched += 1;
                    matched = true;
                    break;
                }
            }

            if !matched {
                remaining.push(txn.clone());
            }
        }

        // Step 2: batch remaining transactions
        let batches: Vec<&[TransactionInput]> = remaining.chunks(self.batch_size).collect();

        // Step 3: call LLM for each batch
        for batch_slice in batches {
            let batch = CategorizationBatch {
                transactions: batch_slice.to_vec(),
                user_overrides: overrides.clone(),
            };

            match self.llm_client.categorize_batch(&batch).await {
                Ok(results) => {
                    for result in results {
                        if result.confidence < 0.7 {
                            flagged += 1;
                        }
                        llm_categorized += 1;
                        all_results.push(result);
                    }
                }
                Err(e) => {
                    tracing::error!("Batch categorization failed: {}", e);
                    failed += batch_slice.len();
                }
            }
        }

        Ok(CategorizationReport {
            results: all_results,
            pattern_matched,
            llm_categorized,
            flagged,
            failed,
        })
    }
}
