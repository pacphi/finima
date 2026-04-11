//! Shared tool-call response parsing for all LLM backends.
//!
//! Both OllamaClient and CandleClient produce OpenAI-compatible tool-call
//! responses. This module extracts the parsing logic so it is written once
//! and tested once.

use rust_decimal::Decimal;

use crate::client::{CategorizationResult, RecurringEnrichment, TransactionInput};
use crate::error::LlmError;

/// Extract tool_calls array from an OpenAI-compatible chat response.
///
/// Looks for `response["message"]["tool_calls"]` (Ollama format) or
/// `response["choices"][0]["message"]["tool_calls"]` (OpenAI format).
pub fn extract_tool_calls(
    response: &serde_json::Value,
) -> Result<Vec<serde_json::Value>, LlmError> {
    // Try Ollama format: response.message.tool_calls
    if let Some(calls) = response
        .get("message")
        .and_then(|m| m.get("tool_calls"))
        .and_then(|tc| tc.as_array())
    {
        return Ok(calls.clone());
    }

    // Try OpenAI format: response.choices[0].message.tool_calls
    if let Some(calls) = response
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("tool_calls"))
        .and_then(|tc| tc.as_array())
    {
        return Ok(calls.clone());
    }

    Err(LlmError::Parse(
        "No tool_calls found in response".to_string(),
    ))
}

/// Extract the `arguments` object from a single tool call.
fn extract_arguments(
    call: &serde_json::Value,
    index: usize,
) -> Result<&serde_json::Value, LlmError> {
    call.get("function")
        .and_then(|f| f.get("arguments"))
        .ok_or_else(|| LlmError::Parse(format!("Missing arguments in tool call {}", index)))
}

/// Parse categorization results from an array of tool calls.
///
/// Each tool call is expected to contain `categorize_transaction` arguments
/// with `transaction_index`, `category`, `subcategory`, `merchant_name`,
/// and `confidence` fields.
pub fn parse_categorization_tool_calls(
    tool_calls: &[serde_json::Value],
    transactions: &[TransactionInput],
) -> Result<Vec<CategorizationResult>, LlmError> {
    let mut results = Vec::with_capacity(tool_calls.len());

    for (i, call) in tool_calls.iter().enumerate() {
        let args = extract_arguments(call, i)?;

        let category = args
            .get("category")
            .and_then(|v| v.as_str())
            .unwrap_or("other")
            .to_string();
        let subcategory = args
            .get("subcategory")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let merchant_name = args
            .get("merchant_name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let confidence = args
            .get("confidence")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);

        // Map back to transaction IDs using the transaction_index field
        // returned by the LLM (1-based). Fall back to positional index if
        // the LLM omits the field.
        let txn_index = args
            .get("transaction_index")
            .and_then(|v| v.as_u64())
            .map(|idx| (idx as usize).saturating_sub(1))
            .unwrap_or(i);

        let transaction_id = transactions.get(txn_index).map(|t| t.id).ok_or_else(|| {
            LlmError::Parse(format!(
                "transaction_index {} (0-based) exceeds batch size {}",
                txn_index,
                transactions.len()
            ))
        })?;

        results.push(CategorizationResult {
            transaction_id,
            category,
            subcategory,
            merchant_name,
            confidence,
        });
    }

    Ok(results)
}

/// Parse enrichment data from the first tool call in the response.
pub fn parse_enrichment_tool_call(
    tool_calls: &[serde_json::Value],
) -> Result<RecurringEnrichment, LlmError> {
    let call = tool_calls
        .first()
        .ok_or_else(|| LlmError::Parse("Empty tool_calls array".to_string()))?;

    let args = extract_arguments(call, 0)?;

    let annual_cost_f64 = args
        .get("annual_cost")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);

    Ok(RecurringEnrichment {
        merchant_full_name: args
            .get("merchant_full_name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        category: args
            .get("category")
            .and_then(|v| v.as_str())
            .unwrap_or("other")
            .to_string(),
        is_subscription: args
            .get("is_subscription")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        is_bill: args
            .get("is_bill")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        is_income: args
            .get("is_income")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        annual_cost: Decimal::from_f64_retain(annual_cost_f64).unwrap_or_default(),
        confidence: args
            .get("confidence")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0),
    })
}

/// Extract plain text content from a chat response (no tool calls).
pub fn extract_content(response: &serde_json::Value) -> Result<String, LlmError> {
    // Ollama format
    if let Some(content) = response
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
    {
        return Ok(content.to_string());
    }

    // OpenAI format
    if let Some(content) = response
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
    {
        return Ok(content.to_string());
    }

    Err(LlmError::Parse("No content in response".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use uuid::Uuid;

    fn make_test_txns(count: usize) -> Vec<TransactionInput> {
        (0..count)
            .map(|i| TransactionInput {
                id: Uuid::new_v4(),
                date: NaiveDate::from_ymd_opt(2026, 4, 1).unwrap(),
                amount: rust_decimal::Decimal::new(-1000, 2),
                description: format!("TEST {}", i),
            })
            .collect()
    }

    #[test]
    fn extract_tool_calls_ollama_format() {
        let response = serde_json::json!({
            "message": {
                "tool_calls": [
                    {"function": {"name": "categorize_transaction", "arguments": {"category": "food_dining"}}}
                ]
            }
        });
        let calls = extract_tool_calls(&response).unwrap();
        assert_eq!(calls.len(), 1);
    }

    #[test]
    fn extract_tool_calls_openai_format() {
        let response = serde_json::json!({
            "choices": [{
                "message": {
                    "tool_calls": [
                        {"function": {"name": "categorize_transaction", "arguments": {"category": "shopping"}}}
                    ]
                }
            }]
        });
        let calls = extract_tool_calls(&response).unwrap();
        assert_eq!(calls.len(), 1);
    }

    #[test]
    fn extract_tool_calls_missing() {
        let response = serde_json::json!({"message": {"content": "hello"}});
        assert!(extract_tool_calls(&response).is_err());
    }

    #[test]
    fn parse_categorization_basic() {
        let txns = make_test_txns(2);
        let tool_calls = vec![
            serde_json::json!({
                "function": {
                    "name": "categorize_transaction",
                    "arguments": {
                        "transaction_index": 1,
                        "category": "food_dining",
                        "subcategory": "restaurants",
                        "merchant_name": "Test Restaurant",
                        "confidence": 0.9
                    }
                }
            }),
            serde_json::json!({
                "function": {
                    "name": "categorize_transaction",
                    "arguments": {
                        "transaction_index": 2,
                        "category": "shopping",
                        "subcategory": "online",
                        "merchant_name": "Amazon",
                        "confidence": 0.85
                    }
                }
            }),
        ];

        let results = parse_categorization_tool_calls(&tool_calls, &txns).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].transaction_id, txns[0].id);
        assert_eq!(results[0].category, "food_dining");
        assert_eq!(results[1].transaction_id, txns[1].id);
        assert_eq!(results[1].category, "shopping");
    }

    #[test]
    fn parse_categorization_fallback_positional() {
        let txns = make_test_txns(1);
        let tool_calls = vec![serde_json::json!({
            "function": {
                "name": "categorize_transaction",
                "arguments": {
                    "category": "utilities",
                    "subcategory": "electric",
                    "merchant_name": "Power Co",
                    "confidence": 0.8
                }
            }
        })];

        let results = parse_categorization_tool_calls(&tool_calls, &txns).unwrap();
        assert_eq!(results[0].transaction_id, txns[0].id);
        assert_eq!(results[0].category, "utilities");
    }

    #[test]
    fn parse_enrichment_basic() {
        let tool_calls = vec![serde_json::json!({
            "function": {
                "name": "enrich_recurring",
                "arguments": {
                    "merchant_full_name": "Netflix",
                    "category": "entertainment",
                    "is_subscription": true,
                    "is_bill": false,
                    "is_income": false,
                    "annual_cost": 119.88,
                    "confidence": 0.95
                }
            }
        })];

        let enrichment = parse_enrichment_tool_call(&tool_calls).unwrap();
        assert_eq!(enrichment.merchant_full_name, "Netflix");
        assert!(enrichment.is_subscription);
        assert!(!enrichment.is_bill);
    }

    #[test]
    fn extract_content_ollama_format() {
        let response = serde_json::json!({
            "message": {"content": "Your spending increased."}
        });
        let content = extract_content(&response).unwrap();
        assert_eq!(content, "Your spending increased.");
    }
}
