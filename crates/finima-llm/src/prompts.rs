use crate::client::{OverridePattern, RecurringGroupCandidate, TransactionInput};

/// Builds the system prompt instructing the LLM to categorize financial transactions.
pub fn build_categorization_system_prompt() -> String {
    r#"You are a financial transaction categorizer. Your job is to classify bank and credit card transactions into the correct category and subcategory.

For each transaction provided, you MUST call the `categorize_transaction` tool exactly once, in the same order as the transactions are listed.

Rules:
- IMPORTANT: Set the `transaction_index` field to the 1-based number of the transaction from the input list.
- Use the normalized, human-readable merchant name (e.g., "Whole Foods Market" not "WHOLEFDS MKT #10432").
- Choose the most specific subcategory you can determine (e.g., "groceries" for grocery stores, "restaurants" for dining out).
- Set confidence between 0.0 and 1.0. Use 0.9+ when the merchant is well-known. Use lower values for ambiguous descriptions.
- If the user has provided override examples, follow those categorizations for matching merchants.
- Negative amounts are expenses; positive amounts are income or refunds.

Available categories: housing, transportation, food_dining, utilities, healthcare, insurance, entertainment, shopping, personal_care, education, travel, gifts_donations, income, transfer, fees_charges, investment, debt_payment, other."#
        .to_string()
}

/// Formats a batch of transactions and user overrides into the user prompt.
pub fn build_categorization_user_prompt(
    transactions: &[TransactionInput],
    overrides: &[OverridePattern],
) -> String {
    let mut prompt = String::new();

    if !overrides.is_empty() {
        prompt.push_str("The user has previously categorized these merchants:\n");
        for o in overrides {
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
