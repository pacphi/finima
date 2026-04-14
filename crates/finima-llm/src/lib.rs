//! LLM client abstraction for Finima.
//!
//! Provides a trait-based interface for transaction categorization
//! via Candle (in-process) or Ollama (HTTP) backends.

pub mod categorizer;
pub mod client;
pub mod enricher;
pub mod error;
pub mod hardware;
pub mod model_download;
pub mod prompts;
pub mod stub;
pub mod tool_calling;
pub mod tool_defs;

#[cfg(feature = "candle")]
pub mod candle_backend;

// Re-export primary types for convenience.
pub use categorizer::{CategorizationProgress, CategorizationReport, Categorizer};
pub use client::{
    CategorizationBatch, CategorizationResult, LlmClient, OverridePattern, RecurringEnrichment,
    RecurringGroupCandidate, RecurringTransactionSummary, TransactionInput,
};
pub use enricher::normalize_merchant;
pub use error::LlmError;
pub use hardware::{detect_hardware, resolve_model, HardwareProfile, ModelSelection};
pub use stub::StubLlmClient;

#[cfg(feature = "ollama")]
pub use client::OllamaClient;

#[cfg(feature = "candle")]
pub use candle_backend::{CandleClient, CandleConfig};

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use chrono::NaiveDate;
    use rust_decimal::Decimal;
    use std::sync::Arc;
    use uuid::Uuid;

    // ---------------------------------------------------------------
    // Mock LlmClient for testing
    // ---------------------------------------------------------------

    struct MockLlmClient;

    #[async_trait]
    impl LlmClient for MockLlmClient {
        async fn categorize_batch(
            &self,
            batch: &CategorizationBatch,
        ) -> Result<Vec<CategorizationResult>, LlmError> {
            Ok(batch
                .transactions
                .iter()
                .map(|t| CategorizationResult {
                    transaction_id: t.id,
                    category: "food_dining".to_string(),
                    subcategory: "restaurants".to_string(),
                    merchant_name: "Mock Merchant".to_string(),
                    confidence: 0.85,
                })
                .collect())
        }

        async fn enrich_recurring(
            &self,
            group: &RecurringGroupCandidate,
        ) -> Result<RecurringEnrichment, LlmError> {
            Ok(RecurringEnrichment {
                merchant_full_name: group.merchant_name.clone(),
                category: "entertainment".to_string(),
                is_subscription: true,
                is_bill: false,
                is_income: false,
                annual_cost: Decimal::new(11988, 2), // 119.88
                confidence: 0.92,
            })
        }

        async fn generate_insight(&self, _prompt: &str) -> Result<String, LlmError> {
            Ok("Your dining spending increased 15% this month.".to_string())
        }
    }

    // A mock that returns low-confidence results for flagging tests.
    struct LowConfidenceMockClient;

    #[async_trait]
    impl LlmClient for LowConfidenceMockClient {
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
                    subcategory: "unknown".to_string(),
                    merchant_name: "Unknown".to_string(),
                    confidence: 0.5,
                })
                .collect())
        }

        async fn enrich_recurring(
            &self,
            _group: &RecurringGroupCandidate,
        ) -> Result<RecurringEnrichment, LlmError> {
            Ok(RecurringEnrichment {
                merchant_full_name: "Unknown".to_string(),
                category: "other".to_string(),
                is_subscription: false,
                is_bill: false,
                is_income: false,
                annual_cost: Decimal::ZERO,
                confidence: 0.5,
            })
        }

        async fn generate_insight(&self, _prompt: &str) -> Result<String, LlmError> {
            Ok("Low confidence insight.".to_string())
        }
    }

    fn make_txn(desc: &str) -> TransactionInput {
        TransactionInput {
            id: Uuid::new_v4(),
            date: NaiveDate::from_ymd_opt(2026, 4, 8).unwrap(),
            amount: Decimal::new(-4250, 2), // -42.50
            description: desc.to_string(),
        }
    }

    fn make_txns(count: usize) -> Vec<TransactionInput> {
        (0..count)
            .map(|i| TransactionInput {
                id: Uuid::new_v4(),
                date: NaiveDate::from_ymd_opt(2026, 4, 1 + (i % 28) as u32).unwrap(),
                amount: Decimal::new(-(1000 + i as i64 * 100), 2),
                description: format!("TEST MERCHANT {}", i),
            })
            .collect()
    }

    // ---------------------------------------------------------------
    // Tool definition tests
    // ---------------------------------------------------------------

    #[test]
    fn tool_def_categorize_is_valid_json() {
        let tool = tool_defs::categorize_transaction_tool();
        assert!(tool.is_object());
        assert_eq!(tool["type"], "function");
        assert_eq!(tool["function"]["name"], "categorize_transaction");
    }

    #[test]
    fn tool_def_contains_all_18_categories() {
        let tool = tool_defs::categorize_transaction_tool();
        let categories = tool["function"]["parameters"]["properties"]["category"]["enum"]
            .as_array()
            .expect("category enum should be an array");
        assert_eq!(categories.len(), 18);

        let expected = tool_defs::all_categories();
        for cat in expected {
            assert!(
                categories.iter().any(|v| v.as_str() == Some(cat)),
                "Missing category: {}",
                cat
            );
        }
    }

    #[test]
    fn tool_def_enrich_is_valid_json() {
        let tool = tool_defs::enrich_recurring_tool();
        assert!(tool.is_object());
        assert_eq!(tool["function"]["name"], "enrich_recurring");
        let required = tool["function"]["parameters"]["required"]
            .as_array()
            .unwrap();
        assert!(required.len() >= 7);
    }

    // ---------------------------------------------------------------
    // Prompt tests
    // ---------------------------------------------------------------

    #[test]
    fn system_prompt_is_nonempty_and_mentions_categorize() {
        let prompt = prompts::build_categorization_system_prompt();
        assert!(!prompt.is_empty());
        assert!(
            prompt.to_lowercase().contains("categorize"),
            "System prompt should mention 'categorize'"
        );
    }

    #[test]
    fn user_prompt_contains_correct_transaction_count() {
        let txns = vec![
            make_txn("STARBUCKS"),
            make_txn("WALMART"),
            make_txn("TARGET"),
        ];
        let prompt = prompts::build_categorization_user_prompt(&txns, &[]);
        assert!(prompt.contains("3 transaction(s)"));
    }

    #[test]
    fn user_prompt_includes_override_examples() {
        let txns = vec![make_txn("STARBUCKS")];
        let overrides = vec![OverridePattern {
            pattern: "STARBUCKS".to_string(),
            category: "food_dining".to_string(),
            subcategory: "coffee".to_string(),
        }];
        let prompt = prompts::build_categorization_user_prompt(&txns, &overrides);
        assert!(prompt.contains("STARBUCKS"));
        assert!(prompt.contains("food_dining > coffee"));
    }

    #[test]
    fn user_prompt_no_overrides_section_when_empty() {
        let txns = vec![make_txn("TEST")];
        let prompt = prompts::build_categorization_user_prompt(&txns, &[]);
        assert!(!prompt.contains("previously categorized"));
    }

    // ---------------------------------------------------------------
    // Pattern matching tests
    // ---------------------------------------------------------------

    #[tokio::test]
    async fn pattern_matching_wholefds() {
        let client = Arc::new(MockLlmClient);
        let categorizer = Categorizer::new(client);

        let txns = vec![make_txn("WHOLEFDS MKT #10432")];
        let overrides = vec![OverridePattern {
            pattern: "WHOLEFDS".to_string(),
            category: "food_dining".to_string(),
            subcategory: "groceries".to_string(),
        }];

        let report = categorizer
            .categorize_transactions(txns, overrides)
            .await
            .unwrap();

        assert_eq!(report.pattern_matched, 1);
        assert_eq!(report.llm_categorized, 0);
        assert_eq!(report.results.len(), 1);
        assert_eq!(report.results[0].category, "food_dining");
        assert_eq!(report.results[0].subcategory, "groceries");
        assert_eq!(report.results[0].confidence, 1.0);
    }

    #[tokio::test]
    async fn pattern_matching_case_insensitive() {
        let client = Arc::new(MockLlmClient);
        let categorizer = Categorizer::new(client);

        let txns = vec![make_txn("wholefds mkt #555")];
        let overrides = vec![OverridePattern {
            pattern: "WHOLEFDS".to_string(),
            category: "food_dining".to_string(),
            subcategory: "groceries".to_string(),
        }];

        let report = categorizer
            .categorize_transactions(txns, overrides)
            .await
            .unwrap();

        assert_eq!(report.pattern_matched, 1);
    }

    #[tokio::test]
    async fn overrides_take_priority_over_llm() {
        let client = Arc::new(MockLlmClient);
        let categorizer = Categorizer::new(client);

        // The mock would return food_dining/restaurants, but the override says shopping/online
        let txns = vec![make_txn("AMZN*1234567"), make_txn("RANDOM STORE")];
        let overrides = vec![OverridePattern {
            pattern: "AMZN".to_string(),
            category: "shopping".to_string(),
            subcategory: "online".to_string(),
        }];

        let report = categorizer
            .categorize_transactions(txns, overrides)
            .await
            .unwrap();

        assert_eq!(report.pattern_matched, 1);
        assert_eq!(report.llm_categorized, 1);
        assert_eq!(report.results.len(), 2);

        // The AMZN transaction should have been pattern-matched, not LLM-categorized
        let amzn_result = report
            .results
            .iter()
            .find(|r| r.category == "shopping")
            .expect("Should have pattern-matched AMZN to shopping");
        assert_eq!(amzn_result.subcategory, "online");
        assert_eq!(amzn_result.confidence, 1.0);
    }

    // ---------------------------------------------------------------
    // Batch chunking tests
    // ---------------------------------------------------------------

    #[tokio::test]
    async fn batch_chunking_45_items() {
        let client = Arc::new(MockLlmClient);
        let categorizer = Categorizer::new(client);

        let txns = make_txns(45);
        let report = categorizer
            .categorize_transactions(txns, vec![])
            .await
            .unwrap();

        // All 45 should be LLM-categorized in 3 batches (20, 20, 5)
        assert_eq!(report.llm_categorized, 45);
        assert_eq!(report.pattern_matched, 0);
        assert_eq!(report.results.len(), 45);
    }

    #[tokio::test]
    async fn batch_chunking_custom_size() {
        let client = Arc::new(MockLlmClient);
        let categorizer = Categorizer::new(client).with_batch_size(15);

        let txns = make_txns(45);
        let report = categorizer
            .categorize_transactions(txns, vec![])
            .await
            .unwrap();

        // 45 items / 15 per batch = 3 batches
        assert_eq!(report.llm_categorized, 45);
        assert_eq!(report.results.len(), 45);
    }

    // ---------------------------------------------------------------
    // Categorization report counts
    // ---------------------------------------------------------------

    #[tokio::test]
    async fn report_counts_mixed_pattern_and_llm() {
        let client = Arc::new(MockLlmClient);
        let categorizer = Categorizer::new(client);

        let txns = vec![
            make_txn("WHOLEFDS MKT #10432"),
            make_txn("SQ *GREENLEAF CAFE"),
            make_txn("UNKNOWN MERCHANT 123"),
        ];
        let overrides = vec![
            OverridePattern {
                pattern: "WHOLEFDS".to_string(),
                category: "food_dining".to_string(),
                subcategory: "groceries".to_string(),
            },
            OverridePattern {
                pattern: "GREENLEAF".to_string(),
                category: "food_dining".to_string(),
                subcategory: "restaurants".to_string(),
            },
        ];

        let report = categorizer
            .categorize_transactions(txns, overrides)
            .await
            .unwrap();

        assert_eq!(report.pattern_matched, 2);
        assert_eq!(report.llm_categorized, 1);
        assert_eq!(report.flagged, 0); // mock returns confidence 0.85
        assert_eq!(report.failed, 0);
        assert_eq!(report.results.len(), 3);
    }

    #[tokio::test]
    async fn flagged_count_for_low_confidence() {
        let client = Arc::new(LowConfidenceMockClient);
        let categorizer = Categorizer::new(client);

        let txns = make_txns(5);
        let report = categorizer
            .categorize_transactions(txns, vec![])
            .await
            .unwrap();

        assert_eq!(report.flagged, 5); // all below 0.7
        assert_eq!(report.llm_categorized, 5);
    }

    // ---------------------------------------------------------------
    // Merchant normalization tests
    // ---------------------------------------------------------------

    #[test]
    fn normalize_sq_prefix() {
        assert_eq!(
            enricher::normalize_merchant("SQ *GREENLEAF CAFE"),
            "Greenleaf Cafe"
        );
    }

    #[test]
    fn normalize_amzn_prefix() {
        assert_eq!(enricher::normalize_merchant("AMZN*1234567"), "Amazon");
    }

    #[test]
    fn normalize_wholefds() {
        assert_eq!(
            enricher::normalize_merchant("WHOLEFDS MKT #10432"),
            "Whole Foods Market"
        );
    }

    #[test]
    fn normalize_tst_prefix() {
        assert_eq!(
            enricher::normalize_merchant("TST*PIZZA PLACE"),
            "Pizza Place"
        );
    }

    #[test]
    fn normalize_titlecase() {
        assert_eq!(
            enricher::normalize_merchant("some random store"),
            "Some Random Store"
        );
    }

    // ---------------------------------------------------------------
    // Mock LlmClient trait object test
    // ---------------------------------------------------------------

    #[tokio::test]
    async fn mock_client_returns_canned_results() {
        let client = MockLlmClient;
        let txn = make_txn("TEST");
        let batch = CategorizationBatch {
            transactions: vec![txn.clone()],
            user_overrides: vec![],
        };

        let results = client.categorize_batch(&batch).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].transaction_id, txn.id);
        assert_eq!(results[0].category, "food_dining");
        assert_eq!(results[0].confidence, 0.85);
    }

    #[tokio::test]
    async fn mock_client_enrich_recurring() {
        let client = MockLlmClient;
        let group = RecurringGroupCandidate {
            merchant_name: "Netflix".to_string(),
            transactions: vec![RecurringTransactionSummary {
                date: NaiveDate::from_ymd_opt(2026, 3, 15).unwrap(),
                amount: Decimal::new(-999, 2),
                description: "NETFLIX.COM".to_string(),
            }],
            frequency_guess: "monthly".to_string(),
        };

        let enrichment = client.enrich_recurring(&group).await.unwrap();
        assert_eq!(enrichment.merchant_full_name, "Netflix");
        assert!(enrichment.is_subscription);
        assert!(!enrichment.is_bill);
    }

    #[tokio::test]
    async fn mock_client_generate_insight() {
        let client = MockLlmClient;
        let insight = client.generate_insight("test data").await.unwrap();
        assert!(!insight.is_empty());
        assert!(insight.contains("dining"));
    }

    // ---------------------------------------------------------------
    // Enrichment prompt test
    // ---------------------------------------------------------------

    #[test]
    fn enrichment_prompt_contains_merchant_info() {
        let group = RecurringGroupCandidate {
            merchant_name: "Netflix".to_string(),
            transactions: vec![RecurringTransactionSummary {
                date: NaiveDate::from_ymd_opt(2026, 3, 15).unwrap(),
                amount: Decimal::new(-999, 2),
                description: "NETFLIX.COM".to_string(),
            }],
            frequency_guess: "monthly".to_string(),
        };

        let prompt = prompts::build_enrichment_prompt(&group);
        assert!(prompt.contains("Netflix"));
        assert!(prompt.contains("monthly"));
        assert!(prompt.contains("NETFLIX.COM"));
    }

    #[test]
    fn insight_prompt_contains_flow_data() {
        let prompt = prompts::build_insight_prompt("Total inflow: $5000, outflow: $4200");
        assert!(prompt.contains("$5000"));
        assert!(prompt.contains("$4200"));
    }
}
