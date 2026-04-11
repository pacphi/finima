use async_trait::async_trait;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::LlmError;

/// A single transaction to be categorized by the LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionInput {
    pub id: Uuid,
    pub date: NaiveDate,
    pub amount: Decimal,
    pub description: String,
}

/// A user-defined override pattern for instant categorization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverridePattern {
    pub pattern: String,
    pub category: String,
    pub subcategory: String,
}

/// A batch of transactions plus user overrides sent to the LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategorizationBatch {
    pub transactions: Vec<TransactionInput>,
    pub user_overrides: Vec<OverridePattern>,
}

/// The categorization result for a single transaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategorizationResult {
    pub transaction_id: Uuid,
    pub category: String,
    pub subcategory: String,
    pub merchant_name: String,
    pub confidence: f64,
}

/// A transaction summary used in recurring group candidates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecurringTransactionSummary {
    pub date: NaiveDate,
    pub amount: Decimal,
    pub description: String,
}

/// A candidate group of recurring transactions for LLM enrichment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecurringGroupCandidate {
    pub merchant_name: String,
    pub transactions: Vec<RecurringTransactionSummary>,
    pub frequency_guess: String,
}

/// Enrichment data returned by the LLM for a recurring group.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecurringEnrichment {
    pub merchant_full_name: String,
    pub category: String,
    pub is_subscription: bool,
    pub is_bill: bool,
    pub is_income: bool,
    pub annual_cost: Decimal,
    pub confidence: f64,
}

/// Trait abstracting the LLM backend.
#[async_trait]
pub trait LlmClient: Send + Sync {
    /// Categorize a batch of transactions using structured tool calling.
    async fn categorize_batch(
        &self,
        batch: &CategorizationBatch,
    ) -> Result<Vec<CategorizationResult>, LlmError>;

    /// Enrich a recurring group candidate with merchant metadata.
    async fn enrich_recurring(
        &self,
        group: &RecurringGroupCandidate,
    ) -> Result<RecurringEnrichment, LlmError>;

    /// Generate a free-form insight from a prompt.
    async fn generate_insight(&self, prompt: &str) -> Result<String, LlmError>;
}

/// Ollama-backed LLM client using the `/api/chat` endpoint.
#[cfg(feature = "ollama")]
pub struct OllamaClient {
    pub base_url: String,
    pub model: String,
    pub http_client: reqwest::Client,
}

#[cfg(feature = "ollama")]
impl OllamaClient {
    pub fn new(base_url: &str, model: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            model: model.to_string(),
            http_client: reqwest::Client::new(),
        }
    }

    /// Maximum number of retry attempts for transient failures.
    const MAX_RETRIES: u32 = 2;

    async fn chat(
        &self,
        messages: Vec<serde_json::Value>,
        tools: Option<Vec<serde_json::Value>>,
    ) -> Result<serde_json::Value, LlmError> {
        let mut body = serde_json::json!({
            "model": self.model,
            "messages": messages,
            "stream": false,
        });

        if let Some(tools) = tools {
            body["tools"] = serde_json::Value::Array(tools);
        }

        let url = format!("{}/api/chat", self.base_url);

        let mut last_error: Option<LlmError> = None;

        for attempt in 0..=Self::MAX_RETRIES {
            if attempt > 0 {
                // Exponential backoff: 1s for first retry, 2s for second.
                let backoff = std::time::Duration::from_secs(1 << (attempt - 1));
                tokio::time::sleep(backoff).await;
            }

            let send_result = self
                .http_client
                .post(&url)
                .json(&body)
                .timeout(std::time::Duration::from_secs(60))
                .send()
                .await;

            let response = match send_result {
                Ok(resp) => resp,
                Err(e) => {
                    // Retry on timeout or connection errors.
                    if e.is_timeout() || e.is_connect() {
                        last_error = Some(if e.is_timeout() {
                            LlmError::Timeout
                        } else {
                            LlmError::Http(e.to_string())
                        });
                        continue;
                    }
                    return Err(LlmError::Http(e.to_string()));
                }
            };

            let status = response.status();

            if status.is_server_error() {
                // 5xx: worth retrying.
                let text = response.text().await.unwrap_or_default();
                last_error = Some(LlmError::Http(format!(
                    "Ollama returned status {}: {}",
                    status, text
                )));
                continue;
            }

            if !status.is_success() {
                // 4xx or other non-retryable errors: fail immediately.
                let text = response.text().await.unwrap_or_default();
                return Err(LlmError::Http(format!(
                    "Ollama returned status {}: {}",
                    status, text
                )));
            }

            let json: serde_json::Value = response
                .json()
                .await
                .map_err(|e| LlmError::Parse(e.to_string()))?;

            return Ok(json);
        }

        // All retries exhausted -- return the last error.
        Err(last_error.unwrap_or_else(|| LlmError::Http("All retries exhausted".to_string())))
    }
}

#[cfg(feature = "ollama")]
#[async_trait]
impl LlmClient for OllamaClient {
    async fn categorize_batch(
        &self,
        batch: &CategorizationBatch,
    ) -> Result<Vec<CategorizationResult>, LlmError> {
        let system_prompt = crate::prompts::build_categorization_system_prompt();
        let user_prompt = crate::prompts::build_categorization_user_prompt(
            &batch.transactions,
            &batch.user_overrides,
        );

        let messages = vec![
            serde_json::json!({"role": "system", "content": system_prompt}),
            serde_json::json!({"role": "user", "content": user_prompt}),
        ];

        let tools = vec![crate::tool_defs::categorize_transaction_tool()];

        let response = self.chat(messages, Some(tools)).await?;

        let tool_calls = crate::tool_calling::extract_tool_calls(&response)?;
        crate::tool_calling::parse_categorization_tool_calls(&tool_calls, &batch.transactions)
    }

    async fn enrich_recurring(
        &self,
        group: &RecurringGroupCandidate,
    ) -> Result<RecurringEnrichment, LlmError> {
        let prompt = crate::prompts::build_enrichment_prompt(group);

        let messages = vec![
            serde_json::json!({
                "role": "system",
                "content": "You are a financial data enrichment assistant. Use the provided tool to return structured metadata about recurring transactions."
            }),
            serde_json::json!({"role": "user", "content": prompt}),
        ];

        let tools = vec![crate::tool_defs::enrich_recurring_tool()];

        let response = self.chat(messages, Some(tools)).await?;

        let tool_calls = crate::tool_calling::extract_tool_calls(&response)?;
        crate::tool_calling::parse_enrichment_tool_call(&tool_calls)
    }

    async fn generate_insight(&self, prompt: &str) -> Result<String, LlmError> {
        let messages = vec![
            serde_json::json!({
                "role": "system",
                "content": "You are a personal finance assistant. Provide clear, actionable insights about the user's financial data."
            }),
            serde_json::json!({"role": "user", "content": prompt}),
        ];

        let response = self.chat(messages, None).await?;

        crate::tool_calling::extract_content(&response)
    }
}
