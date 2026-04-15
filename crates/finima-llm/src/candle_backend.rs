//! In-process LLM inference backend using Candle via mistral.rs.
//!
//! This module provides `CandleClient`, an implementation of `LlmClient`
//! that runs model inference directly in the application process using
//! the mistral.rs engine (built on HuggingFace Candle).
//!
//! Requires the `candle` feature flag to be enabled.

#![cfg(feature = "candle")]

use async_trait::async_trait;
use std::collections::HashMap;
use tracing::info;

use mistralrs::{
    Function, GgufModelBuilder, IsqBits, Model, ModelBuilder, RequestBuilder, TextMessageRole,
    Tool, ToolChoice, ToolType,
};

use crate::client::{
    CategorizationBatch, CategorizationResult, LlmClient, RecurringEnrichment,
    RecurringGroupCandidate,
};
use crate::error::LlmError;
use crate::hardware::{detect_hardware, resolve_model, HardwareProfile, ModelSelection};
use crate::tool_calling;

/// Configuration for the Candle/mistral.rs backend.
#[derive(Debug, Clone)]
pub struct CandleConfig {
    /// HuggingFace model ID for download (e.g., "google/gemma-4-E4B-it").
    pub model_id: String,
    /// Local path to a GGUF file. Overrides `model_id` if non-empty.
    pub model_path: String,
    /// Quantization level (e.g., "Q4_K_M"). Used for ISQ when loading from HF Hub.
    pub quantization: String,
    /// Device selection: "auto", "cuda:0", "metal", "cpu".
    pub device: String,
    /// Maximum context length for inference.
    pub context_length: usize,
    /// Number of CPU threads (0 = auto).
    pub threads: usize,
}

impl Default for CandleConfig {
    fn default() -> Self {
        Self {
            model_id: "google/gemma-4-E4B-it".to_string(),
            model_path: String::new(),
            quantization: "Q4_K_M".to_string(),
            device: "auto".to_string(),
            context_length: 8192,
            threads: 0,
        }
    }
}

/// In-process LLM client using mistral.rs (built on Candle).
///
/// Loads a GGUF or SafeTensors model at construction time and runs
/// inference in the same process with optional GPU acceleration.
pub struct CandleClient {
    model: Model,
    hardware: HardwareProfile,
    model_selection: ModelSelection,
}

impl CandleClient {
    /// Create a new CandleClient, detecting hardware and loading the model.
    ///
    /// This is an expensive operation (5-30s) and should be called once at
    /// application startup, not per-request.
    pub async fn new(config: CandleConfig) -> Result<Self, LlmError> {
        info!("Initializing Candle/mistral.rs inference backend");

        let hardware = detect_hardware();
        info!(?hardware.accelerator, "Hardware detection complete");

        let model_selection = if config.model_id == "auto" || config.model_id.is_empty() {
            resolve_model(&hardware, "auto")
        } else {
            resolve_model(&hardware, &config.model_id)
        };

        info!(?model_selection, "Model resolved");

        let resolved_model_id = match &model_selection {
            ModelSelection::Auto { model_id, .. } => model_id.clone(),
            ModelSelection::Explicit(id) => id.clone(),
        };

        let model = if !config.model_path.is_empty() && config.model_path.ends_with(".gguf") {
            // Load from local GGUF file
            info!(path = %config.model_path, "Loading model from local GGUF file");
            GgufModelBuilder::new(&resolved_model_id, vec![&config.model_path])
                .with_logging()
                .build()
                .await
                .map_err(|e| LlmError::ModelLoad(e.to_string()))?
        } else {
            // Download from HuggingFace Hub with automatic ISQ quantization
            info!(model_id = %resolved_model_id, "Loading model from HuggingFace Hub with ISQ");
            let isq_bits = match config.quantization.as_str() {
                "Q8_0" | "Q8_K" => IsqBits::Eight,
                _ => IsqBits::Four, // Q4_K_M, Q4_0, Q4_K_S, etc.
            };
            ModelBuilder::new(&resolved_model_id)
                .with_auto_isq(isq_bits)
                .with_logging()
                .build()
                .await
                .map_err(|e| LlmError::ModelLoad(e.to_string()))?
        };

        info!("Candle/mistral.rs backend ready for inference");

        Ok(Self {
            model,
            hardware,
            model_selection,
        })
    }

    /// Get the detected hardware profile.
    pub fn hardware(&self) -> &HardwareProfile {
        &self.hardware
    }

    /// Get the resolved model selection.
    pub fn model_selection(&self) -> &ModelSelection {
        &self.model_selection
    }

    /// Convert our JSON tool definitions to mistralrs `Tool` objects.
    fn convert_tools(tool_defs: &[serde_json::Value]) -> Result<Vec<Tool>, LlmError> {
        tool_defs
            .iter()
            .map(|def| {
                let func = def
                    .get("function")
                    .ok_or_else(|| LlmError::Parse("Tool def missing 'function' key".into()))?;

                let name = func
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let description = func
                    .get("description")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let parameters: Option<HashMap<String, serde_json::Value>> = func
                    .get("parameters")
                    .and_then(|v| serde_json::from_value(v.clone()).ok());

                Ok(Tool {
                    tp: ToolType::Function,
                    function: Function {
                        description,
                        name,
                        parameters,
                    },
                })
            })
            .collect()
    }

    /// Convert a `ChatCompletionResponse` to a `serde_json::Value` in the
    /// OpenAI-compatible format expected by our shared `tool_calling` module.
    fn response_to_json(response: &mistralrs::ChatCompletionResponse) -> serde_json::Value {
        let choice = match response.choices.first() {
            Some(c) => c,
            None => return serde_json::json!({"choices": []}),
        };

        let mut message = serde_json::json!({
            "role": choice.message.role,
        });

        if let Some(content) = &choice.message.content {
            message["content"] = serde_json::Value::String(content.clone());
        }

        if let Some(tool_calls) = &choice.message.tool_calls {
            let tc_json: Vec<serde_json::Value> = tool_calls
                .iter()
                .map(|tc| {
                    // CalledFunction.arguments is a JSON string — parse it
                    let args: serde_json::Value =
                        serde_json::from_str(&tc.function.arguments).unwrap_or_default();
                    serde_json::json!({
                        "id": tc.id,
                        "type": "function",
                        "function": {
                            "name": tc.function.name,
                            "arguments": args
                        }
                    })
                })
                .collect();
            message["tool_calls"] = serde_json::Value::Array(tc_json);
        }

        // Return in Ollama-compatible format for our shared parser
        serde_json::json!({
            "message": message
        })
    }

    /// Send a chat request with optional tool calling.
    async fn chat(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        tools: Option<Vec<serde_json::Value>>,
    ) -> Result<serde_json::Value, LlmError> {
        let mut request = RequestBuilder::new()
            .add_message(TextMessageRole::System, system_prompt)
            .add_message(TextMessageRole::User, user_prompt);

        if let Some(tool_defs) = tools {
            let converted = Self::convert_tools(&tool_defs)?;
            request = request
                .set_tools(converted)
                .set_tool_choice(ToolChoice::Auto);
        }

        let response = self
            .model
            .send_chat_request(request)
            .await
            .map_err(|e| LlmError::Inference(e.to_string()))?;

        Ok(Self::response_to_json(&response))
    }
}

#[async_trait]
impl LlmClient for CandleClient {
    async fn categorize_batch(
        &self,
        batch: &CategorizationBatch,
    ) -> Result<Vec<CategorizationResult>, LlmError> {
        // Try batch JSON first -- faster for models that support plain JSON output.
        let json_system = crate::batch_json::build_batch_json_system_prompt(&batch.category_hierarchy);
        let json_user = crate::batch_json::build_batch_json_user_prompt(
            &batch.transactions,
            &batch.user_overrides,
        );

        match self.chat(&json_system, &json_user, None).await {
            Ok(response) => {
                let content = tool_calling::extract_content(&response).unwrap_or_default();
                match crate::batch_json::parse_batch_json_response(&content, &batch.transactions) {
                    Ok(results) if !results.is_empty() => return Ok(results),
                    Ok(_) => {
                        tracing::warn!("Batch JSON returned empty results, falling back to tool-calling");
                    }
                    Err(e) => {
                        tracing::warn!("Batch JSON parse failed, falling back to tool-calling: {}", e);
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Batch JSON chat failed, falling back to tool-calling: {}", e);
            }
        }

        // Fallback: tool-calling protocol (grammar-constrained in Candle)
        let system_prompt =
            crate::prompts::build_categorization_system_prompt(&batch.category_hierarchy);
        let user_prompt = crate::prompts::build_categorization_user_prompt(
            &batch.transactions,
            &batch.user_overrides,
        );

        let tools = vec![crate::tool_defs::categorize_transaction_tool()];

        let response = self.chat(&system_prompt, &user_prompt, Some(tools)).await?;

        let tool_calls = tool_calling::extract_tool_calls(&response)?;
        tool_calling::parse_categorization_tool_calls(&tool_calls, &batch.transactions)
    }

    async fn enrich_recurring(
        &self,
        group: &RecurringGroupCandidate,
    ) -> Result<RecurringEnrichment, LlmError> {
        let system_prompt = "You are a financial data enrichment assistant. Use the provided tool to return structured metadata about recurring transactions.";
        let user_prompt = crate::prompts::build_enrichment_prompt(group);

        let tools = vec![crate::tool_defs::enrich_recurring_tool()];

        let response = self.chat(system_prompt, &user_prompt, Some(tools)).await?;

        let tool_calls = tool_calling::extract_tool_calls(&response)?;
        tool_calling::parse_enrichment_tool_call(&tool_calls)
    }

    async fn generate_insight(&self, prompt: &str) -> Result<String, LlmError> {
        let system_prompt = "You are a personal finance assistant. Provide clear, actionable insights about the user's financial data.";

        let response = self.chat(system_prompt, prompt, None).await?;
        tool_calling::extract_content(&response)
    }
}
