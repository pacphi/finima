//! Recurring payment detection.
//!
//! Groups transactions by normalized merchant name, computes inter-date
//! intervals, classifies frequency, and returns candidates sorted by
//! annual cost descending.

use std::collections::HashMap;

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use finima_core::types::Frequency;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A transaction prepared for analysis (decoupled from the DB model).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionForAnalysis {
    pub id: Uuid,
    pub date: NaiveDate,
    /// Positive = income, negative = expense.
    pub amount: Decimal,
    pub description: String,
    pub merchant_name: Option<String>,
    /// Optional category tag (e.g. "groceries").
    pub category: Option<String>,
    /// Which account this transaction belongs to.
    pub account_id: Option<Uuid>,
}

/// A candidate recurring group surfaced by the detector.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecurringGroupCandidate {
    pub merchant_name: String,
    pub category: Option<String>,
    pub frequency: Frequency,
    pub avg_amount: Decimal,
    pub transaction_count: usize,
    pub first_date: NaiveDate,
    pub last_date: NaiveDate,
    pub next_expected_date: Option<NaiveDate>,
    pub annual_cost: Decimal,
    pub transactions: Vec<Uuid>,
}

// ---------------------------------------------------------------------------
// Detector
// ---------------------------------------------------------------------------

/// Stateless detector — call [`detect_recurring`] directly or instantiate
/// `RecurringDetector` for future configuration hooks.
pub struct RecurringDetector;

impl RecurringDetector {
    pub fn new() -> Self {
        Self
    }

    pub fn detect(&self, transactions: &[TransactionForAnalysis]) -> Vec<RecurringGroupCandidate> {
        detect_recurring(transactions)
    }
}

impl Default for RecurringDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// Normalize a merchant name for grouping: lowercase, strip trailing
/// digits/hashes that banks often append (e.g. "NETFLIX #12345").
fn normalize_merchant(name: &str) -> String {
    let lower = name.to_lowercase();
    // Strip trailing number-like suffixes: #1234, *1234, etc.
    let trimmed = lower
        .trim_end_matches(|c: char| c.is_ascii_digit() || c == '#' || c == '*' || c == ' ')
        .trim();
    trimmed.to_string()
}

/// Classify a set of inter-date intervals (in days) into a [`Frequency`].
fn classify_frequency(intervals: &[i64]) -> Frequency {
    if intervals.is_empty() {
        return Frequency::Variable;
    }
    let avg = intervals.iter().sum::<i64>() as f64 / intervals.len() as f64;

    // Check each pattern with its tolerance.
    if (avg - 1.0).abs() <= 0.5 {
        return Frequency::Daily;
    }
    if (avg - 7.0).abs() <= 1.0 {
        return Frequency::Weekly;
    }
    if (avg - 14.0).abs() <= 2.0 {
        return Frequency::Biweekly;
    }
    if (28.0..=31.0).contains(&avg) || (avg - 30.0).abs() <= 3.0 {
        return Frequency::Monthly;
    }
    if (85.0..=95.0).contains(&avg) || (avg - 90.0).abs() <= 5.0 {
        return Frequency::Quarterly;
    }
    if (175.0..=190.0).contains(&avg) || (avg - 182.0).abs() <= 10.0 {
        return Frequency::Semiannual;
    }
    if (355.0..=375.0).contains(&avg) || (avg - 365.0).abs() <= 15.0 {
        return Frequency::Annual;
    }

    Frequency::Variable
}

/// Return the nominal interval in days for a frequency, used to compute
/// next expected date and annual cost.
fn nominal_interval(freq: Frequency) -> Option<i64> {
    match freq {
        Frequency::Daily => Some(1),
        Frequency::Weekly => Some(7),
        Frequency::Biweekly => Some(14),
        Frequency::Monthly => Some(30),
        Frequency::Quarterly => Some(90),
        Frequency::Semiannual => Some(182),
        Frequency::Annual => Some(365),
        Frequency::Variable => None,
    }
}

/// How many occurrences per year for annual cost estimation.
fn occurrences_per_year(freq: Frequency) -> Decimal {
    match freq {
        Frequency::Daily => Decimal::from(365),
        Frequency::Weekly => Decimal::from(52),
        Frequency::Biweekly => Decimal::from(26),
        Frequency::Monthly => Decimal::from(12),
        Frequency::Quarterly => Decimal::from(4),
        Frequency::Semiannual => Decimal::from(2),
        Frequency::Annual => Decimal::ONE,
        Frequency::Variable => Decimal::from(12), // fallback: assume monthly-ish
    }
}

/// Detect recurring payment groups from a slice of transactions.
///
/// Returns candidates sorted by annual cost (descending).
pub fn detect_recurring(transactions: &[TransactionForAnalysis]) -> Vec<RecurringGroupCandidate> {
    // 1. Group by normalized merchant name.
    let mut groups: HashMap<String, Vec<&TransactionForAnalysis>> = HashMap::new();
    for txn in transactions {
        let key = match &txn.merchant_name {
            Some(name) if !name.is_empty() => normalize_merchant(name),
            _ => normalize_merchant(&txn.description),
        };
        groups.entry(key).or_default().push(txn);
    }

    let mut candidates = Vec::new();

    for (merchant, mut txns) in groups {
        // Need at least 2 transactions to detect recurrence.
        if txns.len() < 2 {
            continue;
        }

        // 2. Sort by date.
        txns.sort_by_key(|t| t.date);

        // 3. Compute inter-date intervals.
        let intervals: Vec<i64> = txns
            .windows(2)
            .map(|w| (w[1].date - w[0].date).num_days())
            .collect();

        // 4. Classify frequency.
        let frequency = classify_frequency(&intervals);

        // 5. Compute statistics.
        let sum: Decimal = txns.iter().map(|t| t.amount.abs()).sum();
        let count = txns.len();
        let avg_amount = sum / Decimal::from(count as i64);
        let first_date = txns.first().unwrap().date;
        let last_date = txns.last().unwrap().date;

        let next_expected_date =
            nominal_interval(frequency).map(|d| last_date + chrono::Duration::days(d));

        let annual_cost = avg_amount * occurrences_per_year(frequency);

        let category = txns.iter().find_map(|t| t.category.clone());

        let transaction_ids: Vec<Uuid> = txns.iter().map(|t| t.id).collect();

        candidates.push(RecurringGroupCandidate {
            merchant_name: merchant,
            category,
            frequency,
            avg_amount,
            transaction_count: count,
            first_date,
            last_date,
            next_expected_date,
            annual_cost,
            transactions: transaction_ids,
        });
    }

    // Sort by annual cost descending.
    candidates.sort_by(|a, b| b.annual_cost.cmp(&a.annual_cost));
    candidates
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use rust_decimal_macros::dec;

    fn txn(id: u128, date: &str, amount: Decimal, merchant: &str) -> TransactionForAnalysis {
        TransactionForAnalysis {
            id: Uuid::from_u128(id),
            date: NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap(),
            amount,
            description: merchant.to_string(),
            merchant_name: Some(merchant.to_string()),
            category: None,
            account_id: None,
        }
    }

    #[test]
    fn monthly_pattern_detected() {
        let txns = vec![
            txn(1, "2025-01-15", dec!(-9.99), "Netflix"),
            txn(2, "2025-02-15", dec!(-9.99), "Netflix"),
            txn(3, "2025-03-15", dec!(-9.99), "Netflix"),
        ];
        let result = detect_recurring(&txns);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].frequency, Frequency::Monthly);
        assert_eq!(result[0].transaction_count, 3);
    }

    #[test]
    fn weekly_pattern_detected() {
        let txns = vec![
            txn(1, "2025-01-01", dec!(-50.00), "Groceries"),
            txn(2, "2025-01-08", dec!(-55.00), "Groceries"),
            txn(3, "2025-01-15", dec!(-48.00), "Groceries"),
        ];
        let result = detect_recurring(&txns);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].frequency, Frequency::Weekly);
    }

    #[test]
    fn quarterly_pattern_detected() {
        let txns = vec![
            txn(1, "2025-01-01", dec!(-100.00), "Insurance Co"),
            txn(2, "2025-04-01", dec!(-100.00), "Insurance Co"),
            txn(3, "2025-07-01", dec!(-100.00), "Insurance Co"),
        ];
        let result = detect_recurring(&txns);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].frequency, Frequency::Quarterly);
    }

    #[test]
    fn annual_pattern_detected() {
        let txns = vec![
            txn(1, "2024-06-01", dec!(-120.00), "Domain Registrar"),
            txn(2, "2025-06-01", dec!(-120.00), "Domain Registrar"),
        ];
        let result = detect_recurring(&txns);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].frequency, Frequency::Annual);
    }

    #[test]
    fn variable_with_mixed_intervals() {
        let txns = vec![
            txn(1, "2025-01-01", dec!(-20.00), "RandomShop"),
            txn(2, "2025-01-10", dec!(-25.00), "RandomShop"),
            txn(3, "2025-02-20", dec!(-22.00), "RandomShop"),
        ];
        let result = detect_recurring(&txns);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].frequency, Frequency::Variable);
    }

    #[test]
    fn single_transaction_not_recurring() {
        let txns = vec![txn(1, "2025-01-01", dec!(-50.00), "OneTimePurchase")];
        let result = detect_recurring(&txns);
        assert!(result.is_empty());
    }

    #[test]
    fn two_transactions_monthly_gap() {
        let txns = vec![
            txn(1, "2025-01-15", dec!(-15.00), "Spotify"),
            txn(2, "2025-02-14", dec!(-15.00), "Spotify"),
        ];
        let result = detect_recurring(&txns);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].frequency, Frequency::Monthly);
        assert_eq!(result[0].transaction_count, 2);
    }

    #[test]
    fn sorted_by_annual_cost_descending() {
        let txns = vec![
            txn(1, "2025-01-01", dec!(-5.00), "CheapSub"),
            txn(2, "2025-02-01", dec!(-5.00), "CheapSub"),
            txn(3, "2025-01-01", dec!(-100.00), "ExpensiveSub"),
            txn(4, "2025-02-01", dec!(-100.00), "ExpensiveSub"),
        ];
        let result = detect_recurring(&txns);
        assert_eq!(result.len(), 2);
        assert!(result[0].annual_cost > result[1].annual_cost);
    }

    #[test]
    fn normalize_strips_trailing_numbers() {
        assert_eq!(normalize_merchant("NETFLIX #12345"), "netflix");
        assert_eq!(normalize_merchant("SPOTIFY *9876"), "spotify");
    }

    #[test]
    fn biweekly_pattern_detected() {
        let txns = vec![
            txn(1, "2025-01-03", dec!(-2000.00), "Payroll"),
            txn(2, "2025-01-17", dec!(-2000.00), "Payroll"),
            txn(3, "2025-01-31", dec!(-2000.00), "Payroll"),
        ];
        let result = detect_recurring(&txns);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].frequency, Frequency::Biweekly);
    }
}
