//! LLM-powered article summarization.

use finima_llm::LlmClient;

/// Errors from the summarization process.
#[derive(Debug, thiserror::Error)]
pub enum SummaryError {
    #[error("LLM error: {0}")]
    Llm(String),
}

/// Generates 2-sentence summaries of articles via an LLM backend.
pub struct ArticleSummarizer;

impl ArticleSummarizer {
    /// Ask the LLM to produce a concise 2-sentence summary of the article.
    pub async fn summarize(
        client: &dyn LlmClient,
        title: &str,
        content: &str,
    ) -> Result<String, SummaryError> {
        let prompt = format!(
            "Summarize this financial article in exactly 2 sentences. \
             Be concise and focus on the key takeaway.\n\n\
             Title: {}\n\n\
             Content: {}",
            title, content
        );

        client
            .generate_insight(&prompt)
            .await
            .map_err(|e| SummaryError::Llm(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use finima_llm::{
        CategorizationBatch, CategorizationResult, LlmError, RecurringEnrichment,
        RecurringGroupCandidate,
    };

    struct MockSummaryClient;

    #[async_trait]
    impl LlmClient for MockSummaryClient {
        async fn categorize_batch(
            &self,
            _batch: &CategorizationBatch,
        ) -> Result<Vec<CategorizationResult>, LlmError> {
            Ok(vec![])
        }

        async fn enrich_recurring(
            &self,
            _group: &RecurringGroupCandidate,
        ) -> Result<RecurringEnrichment, LlmError> {
            Ok(RecurringEnrichment {
                merchant_full_name: String::new(),
                category: "other".to_string(),
                is_subscription: false,
                is_bill: false,
                is_income: false,
                annual_cost: rust_decimal::Decimal::ZERO,
                confidence: 0.0,
            })
        }

        async fn generate_insight(&self, prompt: &str) -> Result<String, LlmError> {
            // Return a canned summary that references the prompt content.
            if prompt.contains("Budget") {
                Ok("Budgeting helps control spending. Start with tracking expenses.".to_string())
            } else {
                Ok(
                    "This article covers financial topics. It provides useful guidance."
                        .to_string(),
                )
            }
        }
    }

    #[tokio::test]
    async fn summarize_returns_two_sentence_summary() {
        let client = MockSummaryClient;
        let summary = ArticleSummarizer::summarize(
            &client,
            "How to Budget in 2026",
            "Budgeting is about tracking income and expenses.",
        )
        .await
        .unwrap();

        assert!(!summary.is_empty());
        assert!(summary.contains("Budgeting"));
    }

    #[tokio::test]
    async fn summarize_generic_article() {
        let client = MockSummaryClient;
        let summary = ArticleSummarizer::summarize(
            &client,
            "Investment Tips",
            "Diversification reduces risk.",
        )
        .await
        .unwrap();

        assert!(!summary.is_empty());
    }
}
