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
    /// `true` if the run was cancelled before completing all batches.
    pub cancelled: bool,
}

/// Progress update emitted after each batch completes.
#[derive(Debug, Clone)]
pub struct CategorizationProgress {
    /// Number of transactions categorized so far.
    pub categorized: usize,
    /// Total number of transactions being processed.
    pub total: usize,
    /// Number of low-confidence results so far.
    pub flagged: usize,
    /// Results from the most recent batch (empty for pattern-match progress).
    /// Allows the caller to persist results incrementally instead of waiting
    /// for the entire run to finish.
    pub batch_results: Vec<CategorizationResult>,
}

/// Orchestrates batch categorization of transactions.
///
/// First applies user override pattern matching (no LLM needed),
/// then sends remaining transactions to the LLM in batches.
pub struct Categorizer {
    llm_client: Arc<dyn LlmClient>,
    batch_size: usize,
    /// Confidence score below which a result is flagged for review.
    confidence_threshold: f64,
    /// Category hierarchy passed through to the system prompt so the LLM
    /// knows the valid subcategory values.
    category_hierarchy: Vec<(String, Vec<String>)>,
}

impl Categorizer {
    /// Default confidence threshold used when none is configured.
    const DEFAULT_CONFIDENCE_THRESHOLD: f64 = 0.7;

    pub fn new(llm_client: Arc<dyn LlmClient>) -> Self {
        Self {
            llm_client,
            batch_size: 20,
            confidence_threshold: Self::DEFAULT_CONFIDENCE_THRESHOLD,
            category_hierarchy: Vec::new(),
        }
    }

    pub fn with_batch_size(mut self, batch_size: usize) -> Self {
        self.batch_size = batch_size;
        self
    }

    /// Set the confidence threshold below which results are flagged.
    pub fn with_confidence_threshold(mut self, threshold: f64) -> Self {
        self.confidence_threshold = threshold;
        self
    }

    /// Set the category hierarchy used in the LLM system prompt.
    pub fn with_category_hierarchy(mut self, hierarchy: Vec<(String, Vec<String>)>) -> Self {
        self.category_hierarchy = hierarchy;
        self
    }

    /// Categorize transactions, calling `on_progress` after each batch.
    ///
    /// The callback returns `true` to continue or `false` to cancel.
    /// Partial results are returned in the report with `cancelled = true`.
    pub async fn categorize_transactions_with_progress<F>(
        &self,
        transactions: Vec<TransactionInput>,
        overrides: Vec<OverridePattern>,
        on_progress: F,
    ) -> Result<CategorizationReport, LlmError>
    where
        F: Fn(&CategorizationProgress) -> bool,
    {
        self.categorize_inner(transactions, overrides, Some(&on_progress))
            .await
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
        self.categorize_inner(
            transactions,
            overrides,
            None::<&fn(&CategorizationProgress) -> bool>,
        )
        .await
    }

    async fn categorize_inner<F>(
        &self,
        transactions: Vec<TransactionInput>,
        overrides: Vec<OverridePattern>,
        on_progress: Option<&F>,
    ) -> Result<CategorizationReport, LlmError>
    where
        F: Fn(&CategorizationProgress) -> bool,
    {
        let mut all_results: Vec<CategorizationResult> = Vec::new();
        let mut pattern_matched: usize = 0;
        let mut llm_categorized: usize = 0;
        let mut flagged: usize = 0;
        let mut failed: usize = 0;
        let mut cancelled = false;

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

        let total = transactions.len();

        // Report pattern-match progress before starting LLM batches.
        if pattern_matched > 0 {
            if let Some(cb) = on_progress {
                let progress = CategorizationProgress {
                    categorized: pattern_matched,
                    total,
                    flagged,
                    batch_results: all_results.clone(),
                };
                if !cb(&progress) {
                    tracing::info!("Categorization cancelled after pattern matching");
                    return Ok(CategorizationReport {
                        results: all_results,
                        pattern_matched,
                        llm_categorized,
                        flagged,
                        failed,
                        cancelled: true,
                    });
                }
            }
        }

        // Step 2: batch remaining transactions
        let batches: Vec<&[TransactionInput]> = remaining.chunks(self.batch_size).collect();

        // Step 3: call LLM for each batch
        let total_batches = batches.len();
        for (batch_num, batch_slice) in batches.into_iter().enumerate() {
            let batch = CategorizationBatch {
                transactions: batch_slice.to_vec(),
                user_overrides: overrides.clone(),
                category_hierarchy: self.category_hierarchy.clone(),
            };

            tracing::info!(
                batch = batch_num + 1,
                total_batches,
                txns = batch_slice.len(),
                "LLM batch starting"
            );

            let batch_start = std::time::Instant::now();
            let mut batch_results = Vec::new();
            match self.llm_client.categorize_batch(&batch).await {
                Ok(results) => {
                    let elapsed = batch_start.elapsed();
                    let secs = elapsed.as_secs();
                    let elapsed_human = if secs >= 60 {
                        format!("{}m {}s", secs / 60, secs % 60)
                    } else {
                        format!("{}s", secs)
                    };
                    tracing::info!(
                        batch = batch_num + 1,
                        total_batches,
                        categorized = results.len(),
                        elapsed = %elapsed_human,
                        "LLM batch complete"
                    );
                    for result in results {
                        if result.confidence < self.confidence_threshold {
                            flagged += 1;
                        }
                        llm_categorized += 1;
                        batch_results.push(result.clone());
                        all_results.push(result);
                    }
                }
                Err(e) => {
                    let elapsed = batch_start.elapsed();
                    let secs = elapsed.as_secs();
                    let elapsed_human = if secs >= 60 {
                        format!("{}m {}s", secs / 60, secs % 60)
                    } else {
                        format!("{}s", secs)
                    };
                    tracing::error!(
                        batch = batch_num + 1,
                        total_batches,
                        elapsed = %elapsed_human,
                        error = %e,
                        "LLM batch failed"
                    );
                    failed += batch_slice.len();
                }
            }

            // Report progress after each batch; stop if callback returns false.
            if let Some(cb) = on_progress {
                let progress = CategorizationProgress {
                    categorized: pattern_matched + llm_categorized + failed,
                    total,
                    flagged,
                    batch_results,
                };
                if !cb(&progress) {
                    tracing::info!(
                        categorized = pattern_matched + llm_categorized,
                        total,
                        "Categorization cancelled mid-run"
                    );
                    cancelled = true;
                    break;
                }
            }
        }

        Ok(CategorizationReport {
            results: all_results,
            pattern_matched,
            llm_categorized,
            flagged,
            failed,
            cancelled,
        })
    }
}
