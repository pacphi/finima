//! Integration tests for finima-llm backends.
//!
//! These tests are ignored by default because they require:
//! - A running Ollama instance (for ollama tests)
//! - A downloaded GGUF model (for candle tests)
//!
//! Run locally:  make test-llm       (auto-starts Ollama, pulls model, runs tests)
//! Run manually: cargo test -p finima-llm --features ollama -- --ignored
//!
//! Environment variables:
//!   OLLAMA_URL         - Ollama endpoint (default: http://localhost:11434)
//!   OLLAMA_TEST_MODEL  - Model to use for tests (default: gemma4:e4b-it-q4_K_M)

use chrono::NaiveDate;
use finima_llm::{CategorizationBatch, LlmClient, StubLlmClient, TransactionInput};
use rust_decimal::Decimal;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_test_batch(descriptions: Vec<&str>) -> CategorizationBatch {
    let transactions = descriptions
        .iter()
        .map(|desc| TransactionInput {
            id: Uuid::new_v4(),
            date: NaiveDate::from_ymd_opt(2026, 4, 11).unwrap(),
            amount: Decimal::new(-2500, 2), // -25.00
            description: desc.to_string(),
        })
        .collect();

    CategorizationBatch {
        transactions,
        user_overrides: vec![],
    }
}

fn make_recurring_group() -> finima_llm::RecurringGroupCandidate {
    use finima_llm::RecurringTransactionSummary;

    finima_llm::RecurringGroupCandidate {
        merchant_name: "Netflix".to_string(),
        transactions: vec![
            RecurringTransactionSummary {
                date: NaiveDate::from_ymd_opt(2026, 1, 15).unwrap(),
                amount: Decimal::new(-1599, 2), // -15.99
                description: "NETFLIX.COM MONTHLY".to_string(),
            },
            RecurringTransactionSummary {
                date: NaiveDate::from_ymd_opt(2026, 2, 15).unwrap(),
                amount: Decimal::new(-1599, 2),
                description: "NETFLIX.COM MONTHLY".to_string(),
            },
            RecurringTransactionSummary {
                date: NaiveDate::from_ymd_opt(2026, 3, 15).unwrap(),
                amount: Decimal::new(-1599, 2),
                description: "NETFLIX.COM MONTHLY".to_string(),
            },
        ],
        frequency_guess: "monthly".to_string(),
    }
}

// ===========================================================================
// 1. Stub Client Tests (always runnable, NOT ignored)
// ===========================================================================

#[tokio::test]
async fn stub_client_categorizes_all_as_other() {
    let client = StubLlmClient;
    let batch = make_test_batch(vec![
        "STARBUCKS STORE #1234",
        "WALMART SUPERCENTER",
        "SHELL OIL 57442",
    ]);

    let results = client.categorize_batch(&batch).await.unwrap();

    assert_eq!(results.len(), 3);
    for result in &results {
        assert_eq!(result.category, "other");
        assert_eq!(result.subcategory, "uncategorized");
        assert_eq!(result.confidence, 0.5);
    }
}

#[tokio::test]
async fn stub_client_enriches_with_defaults() {
    let client = StubLlmClient;
    let group = make_recurring_group();

    let enrichment = client.enrich_recurring(&group).await.unwrap();

    assert_eq!(enrichment.merchant_full_name, "Netflix");
    assert_eq!(enrichment.category, "other");
    assert!(!enrichment.is_subscription);
    assert!(!enrichment.is_bill);
    assert!(!enrichment.is_income);
    assert_eq!(enrichment.annual_cost, Decimal::ZERO);
    assert_eq!(enrichment.confidence, 0.5);
}

#[tokio::test]
async fn stub_client_insight_returns_placeholder() {
    let client = StubLlmClient;

    let insight = client
        .generate_insight("Monthly spending: $3000 on housing, $500 on food")
        .await
        .unwrap();

    assert_eq!(insight, "Insight generation requires a real LLM backend.");
}

// ===========================================================================
// 2. Hardware Detection Tests (always runnable, NOT ignored)
// ===========================================================================

#[test]
fn hardware_detection_returns_valid_profile() {
    use finima_llm::{detect_hardware, HardwareProfile};

    let profile: HardwareProfile = detect_hardware();
    assert!(
        profile.system_ram_mb > 0,
        "System RAM should be greater than 0, got {}",
        profile.system_ram_mb
    );
}

#[test]
fn model_resolution_auto_selects_26b_for_32gb() {
    use finima_llm::hardware::{Accelerator, CpuFeatures};
    use finima_llm::{resolve_model, HardwareProfile, ModelSelection};

    // Test with 32GB -- should pick 26B
    let profile = HardwareProfile {
        accelerator: Accelerator::CpuOnly,
        vram_mb: Some(32_000),
        system_ram_mb: 32_000,
        cpu_features: CpuFeatures::default(),
    };
    match resolve_model(&profile, "auto") {
        ModelSelection::Auto { model_id, .. } => {
            assert!(
                model_id.contains("26B"),
                "Expected 26B model for 32GB, got: {}",
                model_id
            );
        }
        _ => panic!("Expected Auto variant for 'auto' model selection"),
    }
}

#[test]
fn model_resolution_auto_selects_e4b_for_12gb() {
    use finima_llm::hardware::{Accelerator, CpuFeatures};
    use finima_llm::{resolve_model, HardwareProfile, ModelSelection};

    // Test with 12GB -- should pick E4B
    let profile = HardwareProfile {
        accelerator: Accelerator::CpuOnly,
        vram_mb: Some(12_000),
        system_ram_mb: 16_000,
        cpu_features: CpuFeatures::default(),
    };
    match resolve_model(&profile, "auto") {
        ModelSelection::Auto { model_id, .. } => {
            assert!(
                model_id.contains("E4B"),
                "Expected E4B model for 12GB VRAM, got: {}",
                model_id
            );
        }
        _ => panic!("Expected Auto variant for 'auto' model selection"),
    }
}

#[test]
fn model_resolution_auto_selects_e2b_for_4gb() {
    use finima_llm::hardware::{Accelerator, CpuFeatures};
    use finima_llm::{resolve_model, HardwareProfile, ModelSelection};

    // Test with 4GB -- should pick E2B (resource-constrained)
    let profile = HardwareProfile {
        accelerator: Accelerator::CpuOnly,
        vram_mb: None,
        system_ram_mb: 4_000,
        cpu_features: CpuFeatures::default(),
    };
    match resolve_model(&profile, "auto") {
        ModelSelection::Auto { model_id, .. } => {
            assert!(
                model_id.contains("E2B"),
                "Expected E2B model for 4GB RAM, got: {}",
                model_id
            );
        }
        _ => panic!("Expected Auto variant for 'auto' model selection"),
    }
}

#[test]
fn model_resolution_explicit_passthrough() {
    use finima_llm::hardware::{Accelerator, CpuFeatures};
    use finima_llm::{resolve_model, HardwareProfile, ModelSelection};

    let profile = HardwareProfile {
        accelerator: Accelerator::CpuOnly,
        vram_mb: None,
        system_ram_mb: 8_000,
        cpu_features: CpuFeatures::default(),
    };
    match resolve_model(&profile, "my-custom-model") {
        ModelSelection::Explicit(m) => assert_eq!(m, "my-custom-model"),
        _ => panic!("Expected Explicit variant for non-'auto' model"),
    }
}

// ===========================================================================
// 3. Ollama Integration Tests (ignored, requires running Ollama)
// ===========================================================================

#[cfg(feature = "ollama")]
mod ollama_tests {
    use super::*;
    use finima_llm::OllamaClient;

    /// Ollama URL, overridable via `OLLAMA_URL` env var.
    fn ollama_url() -> String {
        std::env::var("OLLAMA_URL").unwrap_or_else(|_| "http://localhost:11434".to_string())
    }

    /// Test model, overridable via `OLLAMA_TEST_MODEL` env var.
    /// Must support tool calling — Gemma 4 is the minimum.
    fn ollama_model() -> String {
        std::env::var("OLLAMA_TEST_MODEL").unwrap_or_else(|_| "gemma4:e4b-it-q4_K_M".to_string())
    }

    /// Check if Ollama is reachable.
    async fn ollama_available() -> bool {
        reqwest::get(format!("{}/api/version", ollama_url()))
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    #[tokio::test]
    #[ignore = "requires running Ollama instance"]
    async fn ollama_categorize_single_transaction() {
        if !ollama_available().await {
            eprintln!("Skipping: Ollama not available at {}", ollama_url());
            return;
        }

        let client = OllamaClient::new(&ollama_url(), &ollama_model());
        let batch = make_test_batch(vec!["STARBUCKS STORE #1234"]);

        let results = client.categorize_batch(&batch).await;
        assert!(
            results.is_ok(),
            "Categorization failed: {:?}",
            results.err()
        );

        let results = results.unwrap();
        assert_eq!(results.len(), 1);
        assert!(!results[0].category.is_empty());
        assert!(results[0].confidence > 0.0);
    }

    #[tokio::test]
    #[ignore = "requires running Ollama instance"]
    async fn ollama_categorize_multiple_transactions() {
        if !ollama_available().await {
            eprintln!("Skipping: Ollama not available at {}", ollama_url());
            return;
        }

        let client = OllamaClient::new(&ollama_url(), &ollama_model());
        let batch = make_test_batch(vec![
            "STARBUCKS STORE #1234",
            "NETFLIX.COM MONTHLY",
            "SHELL OIL 57442",
        ]);

        let results = client.categorize_batch(&batch).await;
        assert!(
            results.is_ok(),
            "Categorization failed: {:?}",
            results.err()
        );

        let results = results.unwrap();
        assert_eq!(results.len(), 3);
        for result in &results {
            assert!(!result.category.is_empty());
            assert!(result.confidence > 0.0);
        }
    }

    #[tokio::test]
    #[ignore = "requires running Ollama instance"]
    async fn ollama_enrich_recurring() {
        if !ollama_available().await {
            eprintln!("Skipping: Ollama not available at {}", ollama_url());
            return;
        }

        let client = OllamaClient::new(&ollama_url(), &ollama_model());
        let group = make_recurring_group();

        let result = client.enrich_recurring(&group).await;
        assert!(result.is_ok(), "Enrichment failed: {:?}", result.err());

        let enrichment = result.unwrap();
        assert!(!enrichment.merchant_full_name.is_empty());
        assert!(!enrichment.category.is_empty());
        assert!(enrichment.confidence > 0.0);
    }

    #[tokio::test]
    #[ignore = "requires running Ollama instance"]
    async fn ollama_generate_insight() {
        if !ollama_available().await {
            eprintln!("Skipping: Ollama not available at {}", ollama_url());
            return;
        }

        let client = OllamaClient::new(&ollama_url(), &ollama_model());
        let result = client
            .generate_insight("Monthly spending: $3000 on housing, $500 on food")
            .await;

        assert!(
            result.is_ok(),
            "Insight generation failed: {:?}",
            result.err()
        );
        assert!(!result.unwrap().is_empty());
    }
}

// ===========================================================================
// 4. Candle Integration Tests (ignored, requires downloaded model)
// ===========================================================================

#[cfg(feature = "candle")]
mod candle_tests {
    use super::*;
    use finima_llm::{CandleClient, CandleConfig};

    #[tokio::test]
    #[ignore = "requires downloaded GGUF model"]
    async fn candle_client_initializes() {
        let config = CandleConfig::default();
        let result = CandleClient::new(config).await;
        // Just test that initialization does not panic
        assert!(
            result.is_ok(),
            "CandleClient init failed: {:?}",
            result.err()
        );
    }

    #[tokio::test]
    #[ignore = "requires downloaded GGUF model"]
    async fn candle_categorize_single_transaction() {
        let config = CandleConfig::default();
        let client = CandleClient::new(config)
            .await
            .expect("CandleClient init failed");

        let batch = make_test_batch(vec!["NETFLIX.COM MONTHLY"]);
        let results = client.categorize_batch(&batch).await;
        assert!(
            results.is_ok(),
            "Categorization failed: {:?}",
            results.err()
        );
    }

    #[tokio::test]
    #[ignore = "requires downloaded GGUF model"]
    async fn candle_enrich_recurring() {
        let config = CandleConfig::default();
        let client = CandleClient::new(config)
            .await
            .expect("CandleClient init failed");

        let group = make_recurring_group();
        let result = client.enrich_recurring(&group).await;
        assert!(result.is_ok(), "Enrichment failed: {:?}", result.err());
    }

    #[tokio::test]
    #[ignore = "requires downloaded GGUF model"]
    async fn candle_generate_insight() {
        let config = CandleConfig::default();
        let client = CandleClient::new(config)
            .await
            .expect("CandleClient init failed");

        let result = client
            .generate_insight("Monthly spending: $3000 on housing, $500 on food")
            .await;
        assert!(
            result.is_ok(),
            "Insight generation failed: {:?}",
            result.err()
        );
    }

    #[tokio::test]
    #[ignore = "requires downloaded GGUF model"]
    async fn candle_hardware_and_model_accessible() {
        let config = CandleConfig::default();
        let client = CandleClient::new(config)
            .await
            .expect("CandleClient init failed");

        let hw = client.hardware();
        assert!(hw.system_ram_mb > 0);

        let selection = client.model_selection();
        // Should have resolved to some model
        match selection {
            finima_llm::ModelSelection::Auto { model_id, .. } => {
                assert!(!model_id.is_empty());
            }
            finima_llm::ModelSelection::Explicit(m) => {
                assert!(!m.is_empty());
            }
        }
    }
}
