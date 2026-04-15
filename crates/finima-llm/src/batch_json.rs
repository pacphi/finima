//! Batch JSON protocol for transaction categorization.
//!
//! An alternative to the per-transaction tool-calling approach.
//! Asks the LLM to return a JSON array of categorization results,
//! which is faster and uses less context than individual tool calls.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::client::{CategorizationBatch, CategorizationResult, OverridePattern, TransactionInput};
use crate::error::LlmError;

/// A single entry in the JSON array returned by the LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchJsonEntry {
    /// 1-based index of the transaction in the input list.
    pub idx: usize,
    /// Category key (e.g. "food_dining").
    pub cat: String,
    /// Subcategory key (e.g. "groceries").
    pub sub: String,
    /// Normalized merchant name (e.g. "Whole Foods Market").
    pub merchant: String,
    /// Confidence score between 0.0 and 1.0.
    pub conf: f64,
}

/// Build the system prompt for the batch JSON protocol.
///
/// Similar to [`crate::prompts::build_categorization_system_prompt`] but
/// instructs the LLM to return a JSON array instead of making tool calls.
pub fn build_batch_json_system_prompt(category_hierarchy: &[(String, Vec<String>)]) -> String {
    let mut prompt = String::from(
        r#"You are a financial transaction categorizer. Your job is to classify bank and credit card transactions into the correct category and subcategory.

You MUST respond with ONLY a JSON array. Each element has these fields:
- "idx": the 1-based index of the transaction from the input list (integer)
- "cat": the category key (string)
- "sub": the subcategory key (string)
- "merchant": the normalized, human-readable merchant name (string)
- "conf": confidence score between 0.0 and 1.0 (number)

Example response format:
[{"idx": 1, "cat": "food_dining", "sub": "groceries", "merchant": "Whole Foods", "conf": 0.95}]

Rules:
- Return one entry per transaction, in the same order as the input.
- Use the normalized, human-readable merchant name (e.g., "Whole Foods Market" not "WHOLEFDS MKT #10432").
- Choose the most specific subcategory from the list below. If none fits, use the category key itself as the subcategory.
- Set confidence 0.9+ when the merchant is well-known. Use lower values for ambiguous descriptions.
- If the user has provided override examples, follow those categorizations for matching merchants.
- Negative amounts are expenses; positive amounts are income or refunds.
- Do NOT include any text outside the JSON array.

"#,
    );

    if category_hierarchy.is_empty() {
        prompt.push_str("Available categories: housing, transportation, food_dining, utilities, healthcare, insurance, entertainment, shopping, personal_care, education, travel, gifts_donations, income, transfer, fees_charges, investment, debt_payment, other.");
    } else {
        prompt.push_str("Available categories and their valid subcategories:\n");
        for (cat, subs) in category_hierarchy {
            if subs.is_empty() {
                prompt.push_str(&format!("- {}\n", cat));
            } else {
                prompt.push_str(&format!("- {}: {}\n", cat, subs.join(", ")));
            }
        }
    }

    prompt
}

/// Build the user prompt for the batch JSON protocol.
///
/// Lists all transactions and relevant overrides, instructing the LLM to
/// return a JSON array.
pub fn build_batch_json_user_prompt(
    transactions: &[TransactionInput],
    overrides: &[OverridePattern],
) -> String {
    let mut prompt = String::new();

    // Only include overrides whose pattern matches at least one transaction.
    let relevant_overrides: Vec<&OverridePattern> = overrides
        .iter()
        .filter(|o| {
            let pattern_lower = o.pattern.to_lowercase();
            transactions
                .iter()
                .any(|txn| txn.description.to_lowercase().contains(&pattern_lower))
        })
        .collect();

    if !relevant_overrides.is_empty() {
        prompt.push_str("The user has previously categorized these merchants:\n");
        for o in &relevant_overrides {
            prompt.push_str(&format!(
                "- \"{}\" => {} > {}\n",
                o.pattern, o.category, o.subcategory
            ));
        }
        prompt.push('\n');
    }

    prompt.push_str(&format!(
        "Categorize the following {} transaction(s). Return a JSON array with one entry per transaction:\n\n",
        transactions.len()
    ));

    for (i, txn) in transactions.iter().enumerate() {
        prompt.push_str(&format!(
            "{}. date={}, amount={}, description=\"{}\"\n",
            i + 1,
            txn.date,
            txn.amount,
            txn.description,
        ));
    }

    prompt
}

/// Parse the LLM's JSON array response into `Vec<CategorizationResult>`.
///
/// The `transactions` slice is used to map 1-based indices back to transaction IDs.
/// Entries with out-of-range indices are skipped with a warning.
pub fn parse_batch_json_response(
    response_text: &str,
    transactions: &[TransactionInput],
) -> Result<Vec<CategorizationResult>, LlmError> {
    // Try to find the JSON array in the response. The LLM might include
    // surrounding text despite instructions.
    let json_str = extract_json_array(response_text).ok_or_else(|| {
        LlmError::Parse(format!(
            "No JSON array found in LLM response: {}",
            truncate(response_text, 200)
        ))
    })?;

    let entries: Vec<BatchJsonEntry> = serde_json::from_str(&json_str).map_err(|e| {
        LlmError::Parse(format!(
            "Failed to parse batch JSON array: {}. Raw: {}",
            e,
            truncate(&json_str, 200)
        ))
    })?;

    let mut results = Vec::with_capacity(entries.len());

    for entry in entries {
        // Handle both 0-based and 1-based indices from the LLM.
        // If idx is 1-based (1..=N), subtract 1. If 0-based (0..N-1), use as-is.
        let txn_index = if entry.idx >= 1 && entry.idx <= transactions.len() {
            entry.idx - 1 // 1-based → 0-based
        } else if entry.idx < transactions.len() {
            entry.idx // already 0-based
        } else {
            tracing::warn!(
                idx = entry.idx,
                total = transactions.len(),
                "Batch JSON entry index out of range, skipping"
            );
            continue;
        };
        let transaction_id: Uuid = transactions[txn_index].id;

        results.push(CategorizationResult {
            transaction_id,
            category: entry.cat,
            subcategory: entry.sub,
            merchant_name: entry.merchant,
            confidence: entry.conf.clamp(0.0, 1.0),
        });
    }

    Ok(results)
}

/// Build the Ollama request body for a batch JSON categorization request.
///
/// Sets `"format": "json"` and `"num_ctx"` to enable constrained JSON
/// decoding and control the context window size.
pub fn build_batch_json_request_body(
    model: &str,
    messages: Vec<serde_json::Value>,
    num_ctx: usize,
) -> serde_json::Value {
    // Use structured output with a JSON schema to constrain output to a
    // valid JSON array. This is critical:
    //
    // - `format: "json"` only outputs a SINGLE JSON value (breaks batching)
    // - No format at all lets the model narrate instead of producing JSON
    // - Schema-based format forces a JSON array of correctly-shaped objects
    //
    // We do NOT set `think: false` here. Some models (Qwen3) need internal
    // reasoning to produce structured output reliably. The schema constraint
    // ensures the final output is clean JSON regardless of thinking.
    serde_json::json!({
        "model": model,
        "messages": messages,
        "stream": false,
        "format": {
            "type": "array",
            "items": {
                "type": "object",
                "required": ["idx", "cat", "sub", "merchant", "conf"],
                "properties": {
                    "idx": { "type": "integer" },
                    "cat": { "type": "string" },
                    "sub": { "type": "string" },
                    "merchant": { "type": "string" },
                    "conf": { "type": "number" }
                }
            }
        },
        "options": {
            "num_ctx": num_ctx
        }
    })
}

/// Extract the first JSON array from a string, handling surrounding text.
fn extract_json_array(text: &str) -> Option<String> {
    // First try: find a JSON array [...]
    if let Some(start) = text.find('[') {
        let mut depth = 0;
        let bytes = text.as_bytes();
        for (i, &byte) in bytes[start..].iter().enumerate() {
            match byte {
                b'[' => depth += 1,
                b']' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(text[start..start + i + 1].to_string());
                    }
                }
                _ => {}
            }
        }
    }

    // Fallback: the LLM returned a single JSON object {...} instead of an array.
    // Wrap it in an array.
    if let Some(start) = text.find('{') {
        let mut depth = 0;
        let bytes = text.as_bytes();
        for (i, &byte) in bytes[start..].iter().enumerate() {
            match byte {
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        let obj = &text[start..start + i + 1];
                        return Some(format!("[{}]", obj));
                    }
                }
                _ => {}
            }
        }
    }

    None
}

/// Truncate a string for error messages.
fn truncate(s: &str, max_len: usize) -> &str {
    if s.len() <= max_len {
        s
    } else {
        &s[..max_len]
    }
}

/// Categorize a batch using the batch JSON protocol via Ollama.
///
/// This is the main entry point for batch JSON categorization. It builds
/// the prompts, sends the request, and parses the response.
#[cfg(feature = "ollama")]
pub async fn categorize_batch_json(
    client: &crate::client::OllamaClient,
    batch: &CategorizationBatch,
    num_ctx: usize,
) -> Result<Vec<CategorizationResult>, LlmError> {
    let system_prompt = build_batch_json_system_prompt(&batch.category_hierarchy);
    let user_prompt = build_batch_json_user_prompt(&batch.transactions, &batch.user_overrides);

    let messages = vec![
        serde_json::json!({"role": "system", "content": system_prompt}),
        serde_json::json!({"role": "user", "content": user_prompt}),
    ];

    let body = build_batch_json_request_body(&client.model, messages, num_ctx);

    let url = format!("{}/api/chat", client.base_url);

    let mut last_error: Option<LlmError> = None;

    for attempt in 0..=client.max_retries {
        if attempt > 0 {
            let backoff = std::time::Duration::from_secs(1 << (attempt - 1));
            tokio::time::sleep(backoff).await;
        }

        let send_result = client
            .http_client
            .post(&url)
            .json(&body)
            .timeout(std::time::Duration::from_secs(client.timeout_seconds))
            .send()
            .await;

        let response = match send_result {
            Ok(resp) => resp,
            Err(e) => {
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
            let text = response.text().await.unwrap_or_default();
            last_error = Some(LlmError::Http(format!(
                "Ollama returned status {}: {}",
                status, text
            )));
            continue;
        }

        if !status.is_success() {
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

        // Extract the content from the assistant message.
        let content = json["message"]["content"].as_str().unwrap_or_default();

        return parse_batch_json_response(content, &batch.transactions);
    }

    Err(last_error.unwrap_or_else(|| LlmError::Http("All retries exhausted".to_string())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use rust_decimal::Decimal;

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

    #[test]
    fn system_prompt_mentions_json_array() {
        let prompt = build_batch_json_system_prompt(&[]);
        assert!(prompt.contains("JSON array"));
        assert!(prompt.contains("idx"));
        assert!(prompt.contains("cat"));
        assert!(prompt.contains("sub"));
    }

    #[test]
    fn system_prompt_includes_hierarchy() {
        let hierarchy = vec![(
            "food_dining".to_string(),
            vec!["groceries".to_string(), "restaurants".to_string()],
        )];
        let prompt = build_batch_json_system_prompt(&hierarchy);
        assert!(prompt.contains("food_dining: groceries, restaurants"));
    }

    #[test]
    fn user_prompt_says_return_json() {
        let txns = make_txns(3);
        let prompt = build_batch_json_user_prompt(&txns, &[]);
        assert!(prompt.contains("3 transaction(s)"));
        assert!(prompt.contains("Return a JSON array"));
    }

    #[test]
    fn parse_valid_json_response() {
        let txns = make_txns(2);
        let json = r#"[{"idx":1,"cat":"food_dining","sub":"groceries","merchant":"Test Store","conf":0.95},{"idx":2,"cat":"shopping","sub":"online","merchant":"Another Store","conf":0.8}]"#.to_string();

        let results = parse_batch_json_response(&json, &txns).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].transaction_id, txns[0].id);
        assert_eq!(results[0].category, "food_dining");
        assert_eq!(results[0].subcategory, "groceries");
        assert_eq!(results[0].confidence, 0.95);
        assert_eq!(results[1].transaction_id, txns[1].id);
        assert_eq!(results[1].category, "shopping");
    }

    #[test]
    fn parse_json_with_surrounding_text() {
        let txns = make_txns(1);
        let response = r#"Here are the results:
[{"idx":1,"cat":"other","sub":"other","merchant":"Unknown","conf":0.5}]
Done!"#;

        let results = parse_batch_json_response(response, &txns).unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn parse_skips_out_of_range_index() {
        let txns = make_txns(1);
        let json = r#"[{"idx":1,"cat":"other","sub":"other","merchant":"A","conf":0.5},{"idx":99,"cat":"other","sub":"other","merchant":"B","conf":0.5}]"#;

        let results = parse_batch_json_response(json, &txns).unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn parse_clamps_confidence() {
        let txns = make_txns(1);
        let json = r#"[{"idx":1,"cat":"other","sub":"other","merchant":"A","conf":1.5}]"#;

        let results = parse_batch_json_response(json, &txns).unwrap();
        assert_eq!(results[0].confidence, 1.0);
    }

    #[test]
    fn parse_error_on_no_json() {
        let txns = make_txns(1);
        let result = parse_batch_json_response("no json here", &txns);
        assert!(result.is_err());
    }

    #[test]
    fn request_body_has_json_format() {
        let body = build_batch_json_request_body("test-model", vec![], 4096);
        // format is now a JSON schema object for structured output
        assert_eq!(body["format"]["type"], "array");
        assert_eq!(body["format"]["items"]["type"], "object");
        let required = body["format"]["items"]["required"].as_array().unwrap();
        assert!(required.contains(&serde_json::json!("idx")));
        assert!(required.contains(&serde_json::json!("cat")));
        assert!(required.contains(&serde_json::json!("conf")));
        assert_eq!(body["options"]["num_ctx"], 4096);
        assert_eq!(body["stream"], false);
    }

    #[test]
    fn user_prompt_includes_overrides() {
        let txns = vec![TransactionInput {
            id: Uuid::new_v4(),
            date: NaiveDate::from_ymd_opt(2026, 4, 1).unwrap(),
            amount: Decimal::new(-4250, 2),
            description: "STARBUCKS #123".to_string(),
        }];
        let overrides = vec![OverridePattern {
            pattern: "STARBUCKS".to_string(),
            category: "food_dining".to_string(),
            subcategory: "coffee".to_string(),
        }];

        let prompt = build_batch_json_user_prompt(&txns, &overrides);
        assert!(prompt.contains("STARBUCKS"));
        assert!(prompt.contains("food_dining > coffee"));
    }
}
