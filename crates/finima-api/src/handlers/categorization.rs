//! Shared categorization pipeline logic.
//!
//! Extracted from the upload-specific pipeline so it can be reused by both
//! the post-upload flow and the on-demand recategorization endpoint.

use std::collections::HashMap;

use uuid::Uuid;

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

    // Step 3: Build transaction inputs
    let transaction_inputs: Vec<TransactionInput> = uncategorized
        .iter()
        .map(|t| TransactionInput {
            id: t.id,
            date: t.date,
            amount: t.amount,
            description: t.description.clone(),
        })
        .collect();

    // Step 4: Categorize using the LLM client
    let llm_client = state
        .llm_client()
        .ok_or_else(|| AppError::ServiceUnavailable("LLM backend not available".to_string()))?;
    let categorizer = Categorizer::new(llm_client);

    // Use the progress-aware variant for WS events + shutdown checks.
    let ws_manager = state.ws_manager().clone();
    let shutdown_state = state.clone();
    let report = categorizer
        .categorize_transactions_with_progress(
            transaction_inputs,
            override_patterns,
            |progress: &CategorizationProgress| {
                // Check for shutdown — return false to cancel the batch loop.
                if shutdown_state.is_shutting_down() {
                    return false;
                }

                if let Some(uid) = upload_id {
                    let ws = ws_manager.clone();
                    let msg = WsMessage::CategorizationProgress {
                        upload_id: uid,
                        categorized: progress.categorized,
                        total: progress.total,
                        flagged: progress.flagged,
                    };
                    // Fire-and-forget from sync context.
                    tokio::spawn(async move {
                        ws.send_to_user(user_id, msg).await;
                    });
                }
                true // continue
            },
        )
        .await
        .map_err(|e| AppError::InternalError(format!("Categorization failed: {}", e)))?;

    // Step 5: Update transaction records with results
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

    // Persist whatever results were collected (may be partial if cancelled).
    if !updates.is_empty() {
        state
            .transaction_repo()
            .update_llm_results(&updates)
            .await?;
    }

    if report.cancelled {
        tracing::warn!(
            account_id = %account_id,
            categorized = report.results.len(),
            "Categorization cancelled due to shutdown — partial results saved"
        );
    }

    let flagged = report.flagged;
    let total = report.results.len();

    // Aggregate category counts for the summary
    let mut category_map: HashMap<String, usize> = HashMap::new();
    for r in &report.results {
        *category_map.entry(r.category.clone()).or_insert(0) += 1;
    }
    let mut categories: Vec<CategoryCount> = category_map
        .into_iter()
        .map(|(category, count)| CategoryCount { category, count })
        .collect();
    categories.sort_by(|a, b| b.count.cmp(&a.count));

    // Skip recurring detection during shutdown to exit promptly.
    if report.cancelled {
        return Ok(Some(CategorizationOutcome {
            total,
            flagged,
            categories,
        }));
    }

    // Step 6: Trigger recurring detection for the portfolio
    let account = state.account_repo().find_by_id(account_id).await?;
    let portfolio_id = account.portfolio_id;

    let (all_txns, _) = state
        .transaction_repo()
        .list(
            &finima_db::TransactionFilters {
                portfolio_id: Some(portfolio_id),
                ..Default::default()
            },
            &finima_db::Pagination {
                page: 1,
                per_page: 10_000,
            },
            &finima_db::Sort::default(),
        )
        .await?;

    let analysis_txns: Vec<finima_analysis::TransactionForAnalysis> = all_txns
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

    let recurring_candidates = finima_analysis::detect_recurring(&analysis_txns);

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
