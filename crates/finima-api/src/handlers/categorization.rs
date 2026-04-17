//! Shared categorization pipeline logic.
//!
//! Extracted from the upload-specific pipeline so it can be reused by both
//! the post-upload flow and the on-demand recategorization endpoint.

use std::collections::HashMap;

use uuid::Uuid;

use finima_categorize::{
    cascade_tiers_0_1, CategoryAssignment as CascadeAssignment, MerchantEntry, MerchantSource,
    PatternEngine,
};
use finima_core::traits::AccountRepo;
use finima_core::AppError;
use finima_db::LlmCategorizationUpdate;
use finima_llm::{CategorizationProgress, Categorizer, OverridePattern, TransactionInput};

use crate::state::AppState;
use crate::ws::{CategoryCount, WsMessage};

/// Result returned after a categorization run completes.
pub struct CategorizationOutcome {
    pub total: usize,
    pub flagged: usize,
    pub categories: Vec<CategoryCount>,
}

/// Run the categorization pipeline for uncategorized transactions in an account.
///
/// Steps:
/// 1. Fetch uncategorized transactions for the account
/// 2. Load user overrides
/// 3. Create a Categorizer and call categorize_transactions
/// 4. Update transaction records with LLM results
/// 5. Trigger recurring detection for the portfolio
///
/// Returns `None` if there are no uncategorized transactions.
pub async fn run_categorization_for_account(
    state: &AppState,
    user_id: Uuid,
    account_id: Uuid,
) -> Result<Option<CategorizationOutcome>, AppError> {
    run_categorization_for_account_with_upload(state, user_id, account_id, None).await
}

/// Run categorization with an optional `upload_id` for per-batch WebSocket
/// progress events during an upload flow.
pub async fn run_categorization_for_account_with_upload(
    state: &AppState,
    user_id: Uuid,
    account_id: Uuid,
    upload_id: Option<Uuid>,
) -> Result<Option<CategorizationOutcome>, AppError> {
    // Step 1: Fetch uncategorized transactions
    let uncategorized = state
        .transaction_repo()
        .find_uncategorized(account_id)
        .await?;

    if uncategorized.is_empty() {
        return Ok(None);
    }

    // Step 2: Load user overrides
    let user_overrides = state.override_repo().list_by_user(user_id).await?;
    let override_patterns: Vec<OverridePattern> = user_overrides
        .iter()
        .map(|o| OverridePattern {
            pattern: o.description_pattern.clone(),
            category: o.category.clone(),
            subcategory: o.subcategory.clone(),
        })
        .collect();

    // ── Step 3: Run Tier 0 + Tier 1 Cascade ──
    //
    // Tier 0 (merchant lookup) uses the shared registry cached on AppState.
    // Tier 1 (pattern engine) is built per-request with default rules.
    // User overrides are applied later by the LLM categorizer.
    let cascade_start = std::time::Instant::now();
    let mut cascade_assignments: Vec<CascadeAssignment> = Vec::new();
    let mut remaining_txn_ids: Vec<Uuid> = Vec::new();

    {
        let registry = state
            .merchant_registry()
            .read()
            .expect("merchant_registry lock poisoned");
        let pattern_engine = PatternEngine::with_defaults();

        for t in &uncategorized {
            // Single entry point for outcome-prefix → Tier 0 → Tier 1. This is
            // the same helper `CascadeEngine::categorize` calls, so fixes land
            // in one place.
            match cascade_tiers_0_1(&registry, &pattern_engine, &t.description, t.amount, None) {
                Some(mut assignment) => {
                    assignment.transaction_id = t.id;
                    cascade_assignments.push(assignment);
                }
                None => remaining_txn_ids.push(t.id),
            }
        }
    }

    let tier0_matched = cascade_assignments
        .iter()
        .filter(|a| a.source_tier == finima_categorize::CategorizationTier::MerchantLookup)
        .count();
    let tier1_matched = cascade_assignments
        .iter()
        .filter(|a| a.source_tier == finima_categorize::CategorizationTier::PatternEngine)
        .count();

    tracing::info!(
        account_id = %account_id,
        total = uncategorized.len(),
        tier0 = tier0_matched,
        tier1 = tier1_matched,
        remaining = remaining_txn_ids.len(),
        cascade_ms = cascade_start.elapsed().as_millis() as u64,
        "cascade categorization (Tier 0 + Tier 1) complete"
    );

    // ── Step 4: Persist cascade results immediately ──
    let mut total_categorized = 0usize;
    let mut flagged = 0usize;
    let mut category_map: HashMap<String, usize> = HashMap::new();
    let confidence_threshold = state.config().llm.confidence_threshold;

    if !cascade_assignments.is_empty() {
        let cascade_updates: Vec<LlmCategorizationUpdate> = cascade_assignments
            .iter()
            .map(|a| LlmCategorizationUpdate {
                transaction_id: a.transaction_id,
                category: a.category.clone(),
                subcategory: a.subcategory.clone(),
                merchant_name: a.merchant_name.clone(),
                llm_confidence: a.confidence,
            })
            .collect();

        state
            .transaction_repo()
            .update_llm_results(&cascade_updates)
            .await?;

        // Set source_tier for cascade results, grouped by tier.
        let mut tier0_ids = Vec::new();
        let mut tier1_ids = Vec::new();
        for a in &cascade_assignments {
            match a.source_tier {
                finima_categorize::CategorizationTier::MerchantLookup => {
                    tier0_ids.push(a.transaction_id);
                }
                finima_categorize::CategorizationTier::PatternEngine => {
                    tier1_ids.push(a.transaction_id);
                }
                _ => {}
            }

            *category_map.entry(a.category.clone()).or_insert(0) += 1;
            if a.confidence < confidence_threshold {
                flagged += 1;
            }
        }

        if !tier0_ids.is_empty() {
            state
                .transaction_repo()
                .set_source_tier(&tier0_ids, "merchant_lookup")
                .await?;
        }
        if !tier1_ids.is_empty() {
            state
                .transaction_repo()
                .set_source_tier(&tier1_ids, "pattern_engine")
                .await?;
        }

        total_categorized += cascade_assignments.len();
    }

    // ── Step 5: Run remaining through LLM (Tier 3) ──
    let llm_cancelled;
    if !remaining_txn_ids.is_empty() {
        let llm_client = match state.llm_client() {
            Some(c) => c,
            None => {
                // No LLM configured or available — skip Tier 3 gracefully.
                // Cascade results from Tiers 0-2 are already persisted above.
                // Remaining transactions keep category = NULL for now.
                tracing::info!(
                    account_id = %account_id,
                    remaining = remaining_txn_ids.len(),
                    "No LLM available — {} transactions left uncategorized (Tiers 0-2 only)",
                    remaining_txn_ids.len()
                );
                if let Some(uid) = upload_id {
                    state.clear_upload_categorization_progress(uid);
                }
                let categories: Vec<CategoryCount> = category_map
                    .into_iter()
                    .map(|(category, count)| CategoryCount { category, count })
                    .collect();
                return Ok(Some(CategorizationOutcome {
                    total: total_categorized,
                    flagged,
                    categories,
                }));
            }
        };

        // Build inputs for only the remaining uncategorized transactions.
        let remaining_set: std::collections::HashSet<Uuid> =
            remaining_txn_ids.iter().copied().collect();
        let transaction_inputs: Vec<TransactionInput> = uncategorized
            .iter()
            .filter(|t| remaining_set.contains(&t.id))
            .map(|t| TransactionInput {
                id: t.id,
                date: t.date,
                amount: t.amount,
                description: t.description.clone(),
            })
            .collect();

        // Build category hierarchy from config for the LLM system prompt.
        let category_hierarchy: Vec<(String, Vec<String>)> = state
            .config()
            .categories
            .iter()
            .map(|c| {
                let subs = c.subcategories.iter().map(|s| s.key.clone()).collect();
                (c.key.clone(), subs)
            })
            .collect();

        let batch_size = state.config().llm.batch_size;
        let confidence_threshold = state.config().llm.confidence_threshold;

        let categorizer = Categorizer::new(llm_client)
            .with_batch_size(batch_size)
            .with_confidence_threshold(confidence_threshold)
            .with_category_hierarchy(category_hierarchy);

        // Use the progress-aware variant for WS events + shutdown checks.
        let ws_manager = state.ws_manager().clone();
        let shutdown_state = state.clone();
        let progress_state = state.clone();
        let persist_state = state.clone();
        // Offset progress by the number already categorized by the cascade so
        // the UI shows accurate totals.
        let cascade_done = cascade_assignments.len();
        let overall_total = uncategorized.len();
        let report = categorizer
            .categorize_transactions_with_progress(
                transaction_inputs,
                override_patterns,
                |progress: &CategorizationProgress| {
                    if shutdown_state.is_shutting_down() {
                        return false;
                    }

                    if !progress.batch_results.is_empty() {
                        let updates: Vec<LlmCategorizationUpdate> = progress
                            .batch_results
                            .iter()
                            .map(|r| LlmCategorizationUpdate {
                                transaction_id: r.transaction_id,
                                category: r.category.clone(),
                                subcategory: r.subcategory.clone(),
                                merchant_name: r.merchant_name.clone(),
                                llm_confidence: r.confidence,
                            })
                            .collect();

                        let ps = persist_state.clone();
                        tokio::spawn(async move {
                            if let Err(e) = ps.transaction_repo().update_llm_results(&updates).await
                            {
                                tracing::error!("Failed to persist batch results: {}", e);
                            }
                        });
                    }

                    if let Some(uid) = upload_id {
                        progress_state.set_upload_categorization_progress(
                            uid,
                            cascade_done + progress.categorized,
                            overall_total,
                        );

                        let ws = ws_manager.clone();
                        let msg = WsMessage::CategorizationProgress {
                            upload_id: uid,
                            categorized: cascade_done + progress.categorized,
                            total: overall_total,
                            flagged: progress.flagged,
                        };
                        tokio::spawn(async move {
                            ws.send_to_user(user_id, msg).await;
                        });
                    }
                    true
                },
            )
            .await
            .map_err(|e| AppError::InternalError(format!("Categorization failed: {}", e)))?;

        if let Some(uid) = upload_id {
            state.clear_upload_categorization_progress(uid);
        }

        // Final persist for LLM results (catches stragglers).
        let updates: Vec<LlmCategorizationUpdate> = report
            .results
            .iter()
            .map(|r| LlmCategorizationUpdate {
                transaction_id: r.transaction_id,
                category: r.category.clone(),
                subcategory: r.subcategory.clone(),
                merchant_name: r.merchant_name.clone(),
                llm_confidence: r.confidence,
            })
            .collect();

        if !updates.is_empty() {
            state
                .transaction_repo()
                .update_llm_results(&updates)
                .await?;
        }

        // Set source_tier = 'llm' for LLM-categorized transactions.
        let llm_ids: Vec<Uuid> = report.results.iter().map(|r| r.transaction_id).collect();
        if !llm_ids.is_empty() {
            state
                .transaction_repo()
                .set_source_tier(&llm_ids, "llm")
                .await?;
        }

        // ── Step 6: Feedback loop — high-confidence LLM results enrich Tier 0 ──
        {
            let registry = state.merchant_registry();
            if let Ok(mut reg) = registry.write() {
                let mut learned = 0usize;
                for r in &report.results {
                    if r.confidence >= 0.9 && !r.merchant_name.is_empty() {
                        reg.add_merchant(MerchantEntry {
                            canonical_name: r.merchant_name.clone(),
                            aliases: vec![],
                            category: r.category.clone(),
                            subcategory: r.subcategory.clone(),
                            confidence: r.confidence,
                            source: MerchantSource::LlmLearned,
                            last_seen: chrono::Utc::now(),
                        });
                        learned += 1;
                    }
                }
                if learned > 0 {
                    tracing::info!(
                        learned,
                        registry_size = reg.len(),
                        "feedback: promoted LLM results to merchant registry"
                    );
                }
            }
        }

        for r in &report.results {
            *category_map.entry(r.category.clone()).or_insert(0) += 1;
            if r.confidence < confidence_threshold {
                flagged += 1;
            }
        }

        total_categorized += report.results.len();
        llm_cancelled = report.cancelled;

        if report.cancelled {
            tracing::warn!(
                account_id = %account_id,
                categorized = report.results.len(),
                "Categorization cancelled due to shutdown — partial results saved"
            );
        }
    } else {
        // All transactions were handled by the cascade; no LLM needed.
        llm_cancelled = false;
        if let Some(uid) = upload_id {
            state.clear_upload_categorization_progress(uid);
        }
        tracing::info!(
            account_id = %account_id,
            total = total_categorized,
            "all transactions categorized by cascade — LLM not needed"
        );
    }

    let total = total_categorized;

    // Aggregate category counts for the summary.
    let mut categories: Vec<CategoryCount> = category_map
        .into_iter()
        .map(|(category, count)| CategoryCount { category, count })
        .collect();
    categories.sort_by_key(|c| std::cmp::Reverse(c.count));

    // Skip recurring detection during shutdown to exit promptly.
    if llm_cancelled {
        return Ok(Some(CategorizationOutcome {
            total,
            flagged,
            categories,
        }));
    }

    // Step 7: Trigger recurring detection for the portfolio
    let account = state.account_repo().find_by_id(account_id).await?;
    let portfolio_id = account.portfolio_id;

    // Pull the whole portfolio (no pagination clamp) so the detector sees the
    // full history — recurring patterns need months of data, not just the
    // most recent 100 rows.
    let analysis_rows = state
        .transaction_repo()
        .list_for_analysis(portfolio_id, None, None)
        .await?;

    let analysis_txns: Vec<finima_analysis::TransactionForAnalysis> = analysis_rows
        .iter()
        .map(|t| finima_analysis::TransactionForAnalysis {
            id: t.id,
            date: t.date,
            amount: t.amount,
            description: t.description.clone(),
            merchant_name: t.merchant_name.clone(),
            category: t.category.clone(),
            account_id: Some(t.account_id),
        })
        .collect();

    let detector_config = finima_analysis::RecurringDetectorConfig::from(state.config().recurring);
    let recurring_candidates =
        finima_analysis::detect_recurring_with_config(&analysis_txns, detector_config);

    // Wipe unconfirmed entries before upserting so candidates that no longer
    // satisfy the detector's thresholds disappear from the UI. Confirmed
    // (user-validated) entries are preserved.
    let _ = state
        .recurring_repo()
        .delete_unconfirmed_by_portfolio(portfolio_id)
        .await;

    for candidate in &recurring_candidates {
        let insert = finima_db::RecurringGroupInsert {
            merchant_name: candidate.merchant_name.clone(),
            category: candidate
                .category
                .clone()
                .unwrap_or_else(|| "other".to_string()),
            frequency: candidate.frequency,
            avg_amount: candidate.avg_amount,
            next_expected_date: candidate.next_expected_date,
            metadata: serde_json::json!({
                "transaction_count": candidate.transaction_count,
                "annual_cost": candidate.annual_cost.to_string(),
            }),
        };
        let _ = state.recurring_repo().upsert(portfolio_id, insert).await;
    }

    if !recurring_candidates.is_empty() {
        state
            .ws_manager()
            .send_to_user(
                user_id,
                WsMessage::RecurringDetected {
                    count: recurring_candidates.len(),
                },
            )
            .await;
    }

    Ok(Some(CategorizationOutcome {
        total,
        flagged,
        categories,
    }))
}
