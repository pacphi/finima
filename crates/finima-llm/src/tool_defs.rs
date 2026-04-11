/// Returns the tool definition JSON for the `categorize_transaction` function.
///
/// This tool schema defines all 18 categories from the domain model,
/// along with subcategory, merchant_name, and confidence fields.
pub fn categorize_transaction_tool() -> serde_json::Value {
    serde_json::json!({
        "type": "function",
        "function": {
            "name": "categorize_transaction",
            "description": "Categorize a financial transaction into a category and subcategory, normalize the merchant name, and provide a confidence score.",
            "parameters": {
                "type": "object",
                "required": ["transaction_index", "category", "subcategory", "merchant_name", "confidence"],
                "properties": {
                    "transaction_index": {
                        "type": "integer",
                        "description": "The 1-based index of the transaction from the input list that this categorization applies to."
                    },
                    "category": {
                        "type": "string",
                        "description": "The top-level spending category.",
                        "enum": [
                            "housing",
                            "transportation",
                            "food_dining",
                            "utilities",
                            "healthcare",
                            "insurance",
                            "entertainment",
                            "shopping",
                            "personal_care",
                            "education",
                            "travel",
                            "gifts_donations",
                            "income",
                            "transfer",
                            "fees_charges",
                            "investment",
                            "debt_payment",
                            "other"
                        ]
                    },
                    "subcategory": {
                        "type": "string",
                        "description": "A more specific classification within the category (e.g., 'groceries', 'restaurants', 'rent')."
                    },
                    "merchant_name": {
                        "type": "string",
                        "description": "The normalized, human-readable merchant name (e.g., 'Whole Foods Market' instead of 'WHOLEFDS MKT #10432')."
                    },
                    "confidence": {
                        "type": "number",
                        "description": "Confidence score between 0.0 and 1.0 indicating how certain the categorization is.",
                        "minimum": 0.0,
                        "maximum": 1.0
                    }
                }
            }
        }
    })
}

/// Returns the tool definition JSON for the `enrich_recurring` function.
///
/// Used to enrich a recurring transaction group with merchant metadata,
/// subscription/bill/income classification, and annual cost estimation.
pub fn enrich_recurring_tool() -> serde_json::Value {
    serde_json::json!({
        "type": "function",
        "function": {
            "name": "enrich_recurring",
            "description": "Enrich a recurring transaction group with merchant metadata including full name, category, subscription/bill/income classification, and estimated annual cost.",
            "parameters": {
                "type": "object",
                "required": ["merchant_full_name", "category", "is_subscription", "is_bill", "is_income", "annual_cost", "confidence"],
                "properties": {
                    "merchant_full_name": {
                        "type": "string",
                        "description": "The full, human-readable merchant or service name."
                    },
                    "category": {
                        "type": "string",
                        "description": "The spending category for this recurring charge.",
                        "enum": [
                            "housing",
                            "transportation",
                            "food_dining",
                            "utilities",
                            "healthcare",
                            "insurance",
                            "entertainment",
                            "shopping",
                            "personal_care",
                            "education",
                            "travel",
                            "gifts_donations",
                            "income",
                            "transfer",
                            "fees_charges",
                            "investment",
                            "debt_payment",
                            "other"
                        ]
                    },
                    "is_subscription": {
                        "type": "boolean",
                        "description": "Whether this is a subscription service (e.g., Netflix, Spotify)."
                    },
                    "is_bill": {
                        "type": "boolean",
                        "description": "Whether this is a recurring bill (e.g., electricity, rent)."
                    },
                    "is_income": {
                        "type": "boolean",
                        "description": "Whether this represents recurring income (e.g., salary, freelance payment)."
                    },
                    "annual_cost": {
                        "type": "number",
                        "description": "Estimated total annual cost based on the observed frequency and amounts."
                    },
                    "confidence": {
                        "type": "number",
                        "description": "Confidence score between 0.0 and 1.0.",
                        "minimum": 0.0,
                        "maximum": 1.0
                    }
                }
            }
        }
    })
}

/// Returns the list of all 18 valid category values.
pub fn all_categories() -> &'static [&'static str] {
    &[
        "housing",
        "transportation",
        "food_dining",
        "utilities",
        "healthcare",
        "insurance",
        "entertainment",
        "shopping",
        "personal_care",
        "education",
        "travel",
        "gifts_donations",
        "income",
        "transfer",
        "fees_charges",
        "investment",
        "debt_payment",
        "other",
    ]
}
