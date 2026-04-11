//! Stub LLM client that returns placeholder results.
//!
//! Use this when the real Candle or Ollama backend is not available.
//! All transactions are categorized as "other" with confidence 0.5.
//! This is clearly marked as a stub and should be replaced with a real
//! LLM client in production.

use async_trait::async_trait;
use rust_decimal::Decimal;

use crate::client::{
    CategorizationBatch, CategorizationResult, LlmClient, RecurringEnrichment,
    RecurringGroupCandidate,
};
use crate::error::LlmError;

/// STUB: Placeholder LLM client that returns canned results.
///
/// Returns category="other", confidence=0.5 for every transaction.
/// Replace with OllamaClient or another real implementation for production use.
pub struct StubLlmClient;

impl StubLlmClient {
    pub fn new() -> Self {
        tracing::warn!(
            "Using STUB LLM client -- all transactions will be categorized as 'other' with confidence 0.5"
        );
        Self
    }
}

impl Default for StubLlmClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LlmClient for StubLlmClient {
    async fn categorize_batch(
        &self,
        batch: &CategorizationBatch,
    ) -> Result<Vec<CategorizationResult>, LlmError> {
        Ok(batch
            .transactions
            .iter()
            .map(|t| CategorizationResult {
                transaction_id: t.id,
                category: "other".to_string(),
                subcategory: "uncategorized".to_string(),
                merchant_name: crate::enricher::normalize_merchant(&t.description),
                confidence: 0.5,
            })
            .collect())
    }

    async fn enrich_recurring(
        &self,
        group: &RecurringGroupCandidate,
    ) -> Result<RecurringEnrichment, LlmError> {
        Ok(RecurringEnrichment {
            merchant_full_name: group.merchant_name.clone(),
            category: "other".to_string(),
            is_subscription: false,
            is_bill: false,
            is_income: false,
            annual_cost: Decimal::ZERO,
            confidence: 0.5,
        })
    }

    async fn generate_insight(&self, _prompt: &str) -> Result<String, LlmError> {
        Ok("Insight generation requires a real LLM backend.".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::TransactionInput;
    use chrono::NaiveDate;
    use rust_decimal::Decimal;
    use uuid::Uuid;

    #[tokio::test]
    async fn stub_returns_other_category() {
        let client = StubLlmClient;
        let batch = CategorizationBatch {
            transactions: vec![TransactionInput {
                id: Uuid::new_v4(),
                date: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
                amount: Decimal::new(-1000, 2),
                description: "TEST STORE".to_string(),
            }],
            user_overrides: vec![],
        };

        let results = client.categorize_batch(&batch).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].category, "other");
        assert_eq!(results[0].confidence, 0.5);
    }
}
