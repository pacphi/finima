//! Budget computation: actual vs. planned and auto-suggestion.

use std::collections::HashMap;

use chrono::{Datelike, NaiveDate};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::recurring::TransactionForAnalysis;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A single budget line item (input).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetEntry {
    pub category: String,
    pub limit: Decimal,
}

/// Result of comparing a budget to actual spending.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetVsActual {
    pub category: String,
    pub limit: Decimal,
    pub spent: Decimal,
    pub remaining: Decimal,
    pub percentage: f64,
}

/// An auto-suggested budget line.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetSuggestion {
    pub category: String,
    pub suggested_limit: Decimal,
    pub avg_monthly: Decimal,
}

// ---------------------------------------------------------------------------
// Functions
// ---------------------------------------------------------------------------

/// Compare budgets to actual spending for a given month.
///
/// `month` should be the first day of the target month (e.g. 2025-03-01).
/// Transactions are filtered to that calendar month.
pub fn compute_budget_vs_actual(
    budgets: &[BudgetEntry],
    transactions: &[TransactionForAnalysis],
    month: NaiveDate,
) -> Vec<BudgetVsActual> {
    let target_year = month.year();
    let target_month = month.month();

    // Sum expenses (negative amounts) by category for the month.
    let mut spent_by_category: HashMap<String, Decimal> = HashMap::new();
    for txn in transactions {
        if txn.date.year() == target_year
            && txn.date.month() == target_month
            && txn.amount < Decimal::ZERO
        {
            if let Some(ref cat) = txn.category {
                *spent_by_category.entry(cat.clone()).or_default() += txn.amount.abs();
            }
        }
    }

    budgets
        .iter()
        .map(|b| {
            let spent = spent_by_category
                .get(&b.category)
                .copied()
                .unwrap_or_default();
            let remaining = b.limit - spent;
            let percentage = if b.limit > Decimal::ZERO {
                spent.to_f64().unwrap_or(0.0) / b.limit.to_f64().unwrap_or(1.0) * 100.0
            } else {
                0.0
            };
            BudgetVsActual {
                category: b.category.clone(),
                limit: b.limit,
                spent,
                remaining,
                percentage,
            }
        })
        .collect()
}

/// Auto-suggest budgets based on average monthly spending over the last N months.
///
/// Rounds each suggestion to the nearest $25.
pub fn auto_suggest_budgets(
    transactions: &[TransactionForAnalysis],
    months: usize,
) -> Vec<BudgetSuggestion> {
    if months == 0 {
        return Vec::new();
    }

    // Accumulate total spending per category across all transactions (expenses only).
    let mut totals: HashMap<String, Decimal> = HashMap::new();
    for txn in transactions {
        if txn.amount < Decimal::ZERO {
            if let Some(ref cat) = txn.category {
                *totals.entry(cat.clone()).or_default() += txn.amount.abs();
            }
        }
    }

    let divisor = Decimal::from(months as i64);
    let twenty_five = Decimal::from(25);

    let mut suggestions: Vec<BudgetSuggestion> = totals
        .into_iter()
        .map(|(category, total)| {
            let avg_monthly = total / divisor;
            // Round up to nearest $25.
            let suggested_limit = round_up_to(avg_monthly, twenty_five);
            BudgetSuggestion {
                category,
                suggested_limit,
                avg_monthly,
            }
        })
        .collect();

    suggestions.sort_by(|a, b| a.category.cmp(&b.category));
    suggestions
}

/// Round `value` up to the nearest multiple of `step`.
fn round_up_to(value: Decimal, step: Decimal) -> Decimal {
    if step.is_zero() {
        return value;
    }
    let remainder = value % step;
    if remainder.is_zero() {
        value
    } else {
        value + (step - remainder)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use rust_decimal_macros::dec;
    use uuid::Uuid;

    fn expense(date: &str, amount: Decimal, category: &str) -> TransactionForAnalysis {
        TransactionForAnalysis {
            id: Uuid::new_v4(),
            date: NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap(),
            amount: -amount.abs(), // ensure negative
            description: format!("{} purchase", category),
            merchant_name: None,
            category: Some(category.to_string()),
            account_id: None,
        }
    }

    #[test]
    fn budget_vs_actual_simple() {
        let budgets = vec![BudgetEntry {
            category: "food".into(),
            limit: dec!(500),
        }];
        let txns = vec![
            expense("2025-03-05", dec!(100), "food"),
            expense("2025-03-20", dec!(150), "food"),
        ];
        let result = compute_budget_vs_actual(
            &budgets,
            &txns,
            NaiveDate::from_ymd_opt(2025, 3, 1).unwrap(),
        );
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].spent, dec!(250));
        assert_eq!(result[0].remaining, dec!(250));
        assert!((result[0].percentage - 50.0).abs() < 0.01);
    }

    #[test]
    fn budget_over_budget() {
        let budgets = vec![BudgetEntry {
            category: "food".into(),
            limit: dec!(200),
        }];
        let txns = vec![
            expense("2025-03-05", dec!(150), "food"),
            expense("2025-03-20", dec!(100), "food"),
        ];
        let result = compute_budget_vs_actual(
            &budgets,
            &txns,
            NaiveDate::from_ymd_opt(2025, 3, 1).unwrap(),
        );
        assert_eq!(result[0].spent, dec!(250));
        assert!(result[0].remaining < Decimal::ZERO);
        assert!(result[0].percentage > 100.0);
    }

    #[test]
    fn budget_ignores_other_months() {
        let budgets = vec![BudgetEntry {
            category: "food".into(),
            limit: dec!(500),
        }];
        let txns = vec![
            expense("2025-02-28", dec!(100), "food"), // previous month
            expense("2025-03-01", dec!(50), "food"),  // target month
        ];
        let result = compute_budget_vs_actual(
            &budgets,
            &txns,
            NaiveDate::from_ymd_opt(2025, 3, 1).unwrap(),
        );
        assert_eq!(result[0].spent, dec!(50));
    }

    #[test]
    fn auto_suggest_rounds_to_25() {
        let txns = vec![
            expense("2025-01-10", dec!(110), "dining"),
            expense("2025-02-10", dec!(130), "dining"),
            expense("2025-03-10", dec!(120), "dining"),
        ];
        let suggestions = auto_suggest_budgets(&txns, 3);
        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0].suggested_limit, dec!(125)); // avg=120 -> 125
    }

    #[test]
    fn auto_suggest_exact_multiple() {
        // Average is exactly $50 -> stays $50
        let txns = vec![
            expense("2025-01-01", dec!(50), "gas"),
            expense("2025-02-01", dec!(50), "gas"),
        ];
        let suggestions = auto_suggest_budgets(&txns, 2);
        assert_eq!(suggestions[0].suggested_limit, dec!(50));
    }
}
