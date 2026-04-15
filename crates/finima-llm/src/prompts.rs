use crate::client::{OverridePattern, RecurringGroupCandidate, TransactionInput};

/// Builds the system prompt instructing the LLM to categorize financial transactions.
///
/// Accepts a list of `(category_key, subcategory_keys)` tuples to dynamically
/// include the valid subcategory values in the prompt.
pub fn build_categorization_system_prompt(category_hierarchy: &[(String, Vec<String>)]) -> String {
    let mut prompt = String::from(
        r#"You are a financial transaction categorizer. Your job is to classify bank and credit card transactions into the correct category and subcategory.

For each transaction provided, you MUST call the `categorize_transaction` tool exactly once, in the same order as the transactions are listed.

Rules:
- IMPORTANT: Set the `transaction_index` field to the 1-based number of the transaction from the input list.
- Use the normalized, human-readable merchant name (e.g., "Whole Foods Market" not "WHOLEFDS MKT #10432").
- Choose the most specific subcategory from the list below. If none fits, use the category key itself as the subcategory.
- Set confidence between 0.0 and 1.0. Use 0.9+ when the merchant is well-known. Use lower values for ambiguous descriptions.
- If the user has provided override examples, follow those categorizations for matching merchants.
- Negative amounts are expenses; positive amounts are income or refunds.

"#,
    );

    if category_hierarchy.is_empty() {
        // Fallback to hardcoded list if no hierarchy provided.
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

/// Formats a batch of transactions and user overrides into the user prompt.
///
/// Only includes overrides whose patterns are relevant to the current batch
/// to keep prompt size small.
pub fn build_categorization_user_prompt(
    transactions: &[TransactionInput],
    overrides: &[OverridePattern],
) -> String {
    let mut prompt = String::new();

    // Only include overrides whose pattern matches at least one transaction in this batch.
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
        "Categorize the following {} transaction(s). Call the categorize_transaction tool once per transaction, in order:\n\n",
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

/// Builds the prompt for enriching a recurring group candidate.
pub fn build_enrichment_prompt(candidate: &RecurringGroupCandidate) -> String {
    let mut prompt = format!(
        "Analyze this recurring transaction group and provide enrichment metadata.\n\n\
         Merchant: {}\n\
         Detected frequency: {}\n\
         Transactions:\n",
        candidate.merchant_name, candidate.frequency_guess,
    );

    for txn in &candidate.transactions {
        prompt.push_str(&format!(
            "  - date={}, amount={}, description=\"{}\"\n",
            txn.date, txn.amount, txn.description,
        ));
    }

    prompt.push_str(
        "\nCall the enrich_recurring tool with the full merchant name, category, \
         whether it's a subscription/bill/income, estimated annual cost, and your confidence.",
    );

    prompt
}

/// Builds a prompt for generating a financial insight from flow data.
pub fn build_insight_prompt(flow_data: &str) -> String {
    format!(
        "Based on the following account flow data, provide a concise, actionable financial insight \
         for the user. Focus on trends, anomalies, or opportunities to save money.\n\n{}",
        flow_data,
    )
}
