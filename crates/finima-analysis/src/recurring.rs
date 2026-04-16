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

/// Configuration knobs for [`RecurringDetector`].
///
/// `min_occurrences_for_variable` and `variable_window_months` together guard
/// the **variable** classification: a candidate whose intervals don't fit any
/// fixed cadence must occur at least `min_occurrences_for_variable` times
/// inside a `variable_window_months`-month sliding window (anchored on the
/// candidate's `last_date`) to be kept. Below that threshold it's treated as
/// noise and dropped.
#[derive(Debug, Clone, Copy)]
pub struct RecurringDetectorConfig {
    pub min_occurrences_for_variable: usize,
    pub variable_window_months: u32,
}

impl RecurringDetectorConfig {
    pub const DEFAULT_MIN_OCCURRENCES_FOR_VARIABLE: usize = 3;
    pub const DEFAULT_VARIABLE_WINDOW_MONTHS: u32 = 6;
}

impl Default for RecurringDetectorConfig {
    fn default() -> Self {
        Self {
            min_occurrences_for_variable: Self::DEFAULT_MIN_OCCURRENCES_FOR_VARIABLE,
            variable_window_months: Self::DEFAULT_VARIABLE_WINDOW_MONTHS,
        }
    }
}

/// Detector configured with thresholds for variable-frequency filtering.
pub struct RecurringDetector {
    config: RecurringDetectorConfig,
}

impl RecurringDetector {
    pub fn new() -> Self {
        Self::with_config(RecurringDetectorConfig::default())
    }

    pub fn with_config(config: RecurringDetectorConfig) -> Self {
        Self { config }
    }

    pub fn detect(&self, transactions: &[TransactionForAnalysis]) -> Vec<RecurringGroupCandidate> {
        detect_recurring_with_config(transactions, self.config)
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
///
/// Uses the **median** rather than the mean so a few outliers — same-day NSF
/// retries, an end-of-life payoff, etc. — don't pull a clearly periodic
/// pattern into Variable.
fn classify_frequency(intervals: &[i64]) -> Frequency {
    let typical = match median(intervals) {
        Some(m) => m,
        None => return Frequency::Variable,
    };

    // Check each pattern with its tolerance.
    if (typical - 1.0).abs() <= 0.5 {
        return Frequency::Daily;
    }
    if (typical - 7.0).abs() <= 1.0 {
        return Frequency::Weekly;
    }
    if (typical - 14.0).abs() <= 2.0 {
        return Frequency::Biweekly;
    }
    if (28.0..=31.0).contains(&typical) || (typical - 30.0).abs() <= 3.0 {
        return Frequency::Monthly;
    }
    if (85.0..=95.0).contains(&typical) || (typical - 90.0).abs() <= 5.0 {
        return Frequency::Quarterly;
    }
    if (175.0..=190.0).contains(&typical) || (typical - 182.0).abs() <= 10.0 {
        return Frequency::Semiannual;
    }
    if (355.0..=375.0).contains(&typical) || (typical - 365.0).abs() <= 15.0 {
        return Frequency::Annual;
    }

    Frequency::Variable
}

fn median(values: &[i64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted: Vec<i64> = values.to_vec();
    sorted.sort_unstable();
    let n = sorted.len();
    Some(if n % 2 == 1 {
        sorted[n / 2] as f64
    } else {
        (sorted[n / 2 - 1] + sorted[n / 2]) as f64 / 2.0
    })
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

/// Detect recurring payment groups from a slice of transactions using the
/// default [`RecurringDetectorConfig`].
///
/// Returns candidates sorted by annual cost (descending).
pub fn detect_recurring(transactions: &[TransactionForAnalysis]) -> Vec<RecurringGroupCandidate> {
    detect_recurring_with_config(transactions, RecurringDetectorConfig::default())
}

/// Detect recurring payment groups using the supplied configuration.
pub fn detect_recurring_with_config(
    transactions: &[TransactionForAnalysis],
    config: RecurringDetectorConfig,
) -> Vec<RecurringGroupCandidate> {
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
        let sum: Decimal = txns.iter().map(|t| t.amount).sum();
        let count = txns.len();
        let avg_amount = sum / Decimal::from(count as i64);
        let first_date = txns.first().unwrap().date;
        let last_date = txns.last().unwrap().date;

        // 6. For Variable frequency, require enough recent occurrences in a
        //    sliding window so we don't surface noisy one-offs.
        if frequency == Frequency::Variable {
            let window_start =
                last_date - chrono::Duration::days((config.variable_window_months as i64) * 30);
            let recent_count = txns.iter().filter(|t| t.date > window_start).count();
            if recent_count < config.min_occurrences_for_variable {
                continue;
            }
        }

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

    // Sort by annual cost (absolute value) descending.
    candidates.sort_by(|a, b| b.annual_cost.abs().cmp(&a.annual_cost.abs()));
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
        assert!(result[0].annual_cost.abs() > result[1].annual_cost.abs());
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

    #[test]
    fn variable_below_threshold_is_dropped() {
        // Two erratic-interval transactions: classifies as Variable, fewer than
        // the default 3-in-6-months minimum, so the candidate should be dropped.
        let txns = vec![
            txn(1, "2025-01-01", dec!(-20.00), "OneOff"),
            txn(2, "2025-04-15", dec!(-200.00), "OneOff"),
        ];
        let result = detect_recurring(&txns);
        assert!(result.is_empty());
    }

    #[test]
    fn variable_outside_window_is_dropped() {
        // Three transactions but two are older than the 6-month sliding window
        // anchored on the latest transaction; only one occurrence is recent.
        let txns = vec![
            txn(1, "2024-01-01", dec!(-20.00), "Sporadic"),
            txn(2, "2024-02-15", dec!(-200.00), "Sporadic"),
            txn(3, "2025-06-01", dec!(-50.00), "Sporadic"),
        ];
        let result = detect_recurring(&txns);
        assert!(result.is_empty());
    }

    #[test]
    fn variable_threshold_can_be_overridden() {
        // With a higher threshold even three recent variable-interval txns
        // get filtered out.
        let txns = vec![
            txn(1, "2025-01-01", dec!(-20.00), "Sometimes"),
            txn(2, "2025-01-10", dec!(-25.00), "Sometimes"),
            txn(3, "2025-02-20", dec!(-22.00), "Sometimes"),
        ];
        let strict = detect_recurring_with_config(
            &txns,
            RecurringDetectorConfig {
                min_occurrences_for_variable: 4,
                variable_window_months: 6,
            },
        );
        assert!(strict.is_empty());

        // With the default config (min = 3) the same input is kept.
        let lenient = detect_recurring(&txns);
        assert_eq!(lenient.len(), 1);
        assert_eq!(lenient[0].frequency, Frequency::Variable);
    }

    #[test]
    fn cornerstone_like_monthly_pattern_classified_correctly() {
        // Reproduces the Cornerstone Bank scenario: ~monthly $1298.77 payments
        // with two same-day NSF $10 entries and a one-off large payment at the
        // end. Despite the noise, the dominant cadence is monthly and the
        // group should not be misclassified as weekly.
        let dates = [
            "2025-05-21",
            "2025-05-21", // NSF $10
            "2025-06-23",
            "2025-07-22",
            "2025-08-21",
            "2025-09-23",
            "2025-10-21",
            "2025-11-21",
            "2025-12-31",
            "2026-01-21",
            "2026-02-23",
            "2026-03-23", // NSF $10
            "2026-03-23",
            "2026-04-07", // one-off larger payoff
        ];
        let amounts = [
            dec!(-1298.77),
            dec!(-10.00),
            dec!(-1298.77),
            dec!(-1298.77),
            dec!(-1298.77),
            dec!(-1298.77),
            dec!(-1298.77),
            dec!(-1298.77),
            dec!(-1298.77),
            dec!(-1298.77),
            dec!(-1298.77),
            dec!(-10.00),
            dec!(-1298.77),
            dec!(-51994.80),
        ];
        let txns: Vec<TransactionForAnalysis> = dates
            .iter()
            .zip(amounts.iter())
            .enumerate()
            .map(|(i, (d, a))| txn(i as u128 + 1, d, *a, "Cornerstone Bank"))
            .collect();
        let result = detect_recurring(&txns);
        assert_eq!(result.len(), 1, "expected one recurring group");
        // Median of intervals lands in the monthly band (~30d), so this
        // dominantly-monthly pattern is classified correctly despite the
        // same-day NSF retries and the one-off payoff.
        assert_eq!(result[0].frequency, Frequency::Monthly);
    }
}
