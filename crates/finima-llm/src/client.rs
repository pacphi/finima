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
    /// Category hierarchy: `(category_key, [subcategory_keys])`.
    /// Used to build the system prompt with valid subcategory values.
    #[serde(default)]
    pub category_hierarchy: Vec<(String, Vec<String>)>,
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
    /// Warm up the backend so the first real request doesn't pay cold-start
    /// latency. For Ollama this loads the model into GPU memory; for Candle
    /// the model is already loaded in-process so this is a no-op.
    async fn warmup(&self) -> Result<(), LlmError> {
        Ok(()) // Default no-op; providers override if needed.
    }

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

/// Options for the Ollama chat request beyond the standard fields.
#[cfg(feature = "ollama")]
#[derive(Debug, Clone)]
pub struct ChatOptions {
    /// Context window size in tokens. Default: 4096.
    pub num_ctx: usize,
    /// Enable constrained JSON output mode. Default: false.
    pub json_format: bool,
}

#[cfg(feature = "ollama")]
impl Default for ChatOptions {
    fn default() -> Self {
        Self {
            num_ctx: 4096,
            json_format: false,
        }
    }
}

/// Ollama-backed LLM client using the `/api/chat` endpoint.
#[cfg(feature = "ollama")]
pub struct OllamaClient {
    pub base_url: String,
    pub model: String,
    pub http_client: reqwest::Client,
    pub timeout_seconds: u64,
    pub max_retries: u32,
}

#[cfg(feature = "ollama")]
impl OllamaClient {
    pub fn new(base_url: &str, model: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            model: model.to_string(),
            http_client: reqwest::Client::new(),
            timeout_seconds: 60,
            max_retries: 2,
        }
    }

    /// Create a new client with configurable timeout and retry settings.
    pub fn with_config(
        base_url: &str,
        model: &str,
        timeout_seconds: u64,
        max_retries: u32,
    ) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            model: model.to_string(),
            http_client: reqwest::Client::new(),
            timeout_seconds,
            max_retries,
        }
    }

    /// Categorize multiple batches in parallel using the batch JSON protocol.
    ///
    /// Sends up to `parallel` concurrent requests to Ollama, each processing
    /// one batch independently. Returns results in the same order as the input
    /// batches.
    pub async fn categorize_batch_parallel(
        &self,
        batches: Vec<CategorizationBatch>,
        parallel: usize,
        num_ctx: usize,
    ) -> Vec<Result<Vec<CategorizationResult>, LlmError>> {
        use tokio::task::JoinSet;

        let parallel = parallel.max(1);
        let total = batches.len();
        let mut results: Vec<Option<Result<Vec<CategorizationResult>, LlmError>>> =
            (0..total).map(|_| None).collect();

        // Process batches in chunks of `parallel`.
        let mut offset = 0;
        while offset < total {
            let chunk_end = (offset + parallel).min(total);
            let mut join_set = JoinSet::new();

            // Spawn up to `parallel` tasks.
            for (batch_idx, batch) in batches[offset..chunk_end].iter().enumerate() {
                let batch_idx = offset + batch_idx;
                let batch = batch.clone();
                let base_url = self.base_url.clone();
                let model = self.model.clone();
                let http_client = self.http_client.clone();
                let timeout_seconds = self.timeout_seconds;
                let max_retries = self.max_retries;
                let ctx = num_ctx;

                join_set.spawn(async move {
                    let client = OllamaClient {
                        base_url,
                        model,
                        http_client,
                        timeout_seconds,
                        max_retries,
                    };
                    let result =
                        crate::batch_json::categorize_batch_json(&client, &batch, ctx).await;
                    (batch_idx, result)
                });
            }

            // Collect results.
            while let Some(join_result) = join_set.join_next().await {
                match join_result {
                    Ok((idx, result)) => {
                        results[idx] = Some(result);
                    }
                    Err(e) => {
                        tracing::error!("Parallel batch task panicked: {}", e);
                    }
                }
            }

            offset = chunk_end;
        }

        // Convert Option<Result> -> Result, filling in errors for any missing.
        results
            .into_iter()
            .map(|opt| {
                opt.unwrap_or_else(|| Err(LlmError::Http("Batch task failed to complete".into())))
            })
            .collect()
    }

    async fn chat(
        &self,
        messages: Vec<serde_json::Value>,
        tools: Option<Vec<serde_json::Value>>,
    ) -> Result<serde_json::Value, LlmError> {
        self.chat_with_options(messages, tools, None).await
    }

    /// Send a chat request with optional format and num_ctx overrides.
    async fn chat_with_options(
        &self,
        messages: Vec<serde_json::Value>,
        tools: Option<Vec<serde_json::Value>>,
        options: Option<ChatOptions>,
    ) -> Result<serde_json::Value, LlmError> {
        let opts = options.unwrap_or_default();

        let mut body = serde_json::json!({
            "model": self.model,
            "messages": messages,
            "stream": false,
            "options": {
                "num_ctx": opts.num_ctx
            }
        });

        if let Some(tools) = tools {
            body["tools"] = serde_json::Value::Array(tools);
            // Only disable thinking for tool-calling mode where Gemma 4
            // wastes tokens on internal reasoning. For plain chat/JSON
            // output, some models (Qwen3) need thinking to follow instructions.
            body["think"] = serde_json::json!(false);
        }

        if opts.json_format {
            body["format"] = serde_json::json!("json");
        }

        let url = format!("{}/api/chat", self.base_url);

        let mut last_error: Option<LlmError> = None;

        for attempt in 0..=self.max_retries {
            if attempt > 0 {
                // Exponential backoff: 1s for first retry, 2s for second.
                let backoff = std::time::Duration::from_secs(1 << (attempt - 1));
                tokio::time::sleep(backoff).await;
            }

            let send_result = self
                .http_client
                .post(&url)
                .json(&body)
                .timeout(std::time::Duration::from_secs(self.timeout_seconds))
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
    async fn warmup(&self) -> Result<(), LlmError> {
        tracing::info!(model = %self.model, "Warming up Ollama model...");
        let messages = vec![serde_json::json!({
            "role": "user",
            "content": "Hi"
        })];
        // Send a tiny request to force model loading into GPU memory.
        // Use a minimal context window and ignore the response content.
        let mut body = serde_json::json!({
            "model": self.model,
            "messages": messages,
            "stream": false,
            "options": { "num_ctx": 128, "num_predict": 1 }
        });
        body["think"] = serde_json::json!(false);

        let url = format!("{}/api/chat", self.base_url);
        let _ = self
            .http_client
            .post(&url)
            .json(&body)
            .timeout(std::time::Duration::from_secs(120))
            .send()
            .await
            .map_err(|e| LlmError::Http(e.to_string()))?;

        tracing::info!(model = %self.model, "Ollama model warm and ready");
        Ok(())
    }

    async fn categorize_batch(
        &self,
        batch: &CategorizationBatch,
    ) -> Result<Vec<CategorizationResult>, LlmError> {
        // Use the batch JSON protocol -- faster and more reliable than tool-calling.
        // Falls back to tool-calling if batch JSON parsing fails.
        match crate::batch_json::categorize_batch_json(self, batch, 4096).await {
            Ok(results) => return Ok(results),
            Err(e) => {
                tracing::warn!(
                    "Batch JSON categorization failed, falling back to tool-calling: {}",
                    e
                );
            }
        }

        // Fallback: tool-calling protocol
        let system_prompt =
            crate::prompts::build_categorization_system_prompt(&batch.category_hierarchy);
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
