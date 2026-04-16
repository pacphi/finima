//! Infers a [`SignConvention`] for an account from a sample of its
//! transaction rows.
//!
//! ## When this runs
//!
//! The import pipeline consults [`SignAutodetector`] only when no other
//! source has produced a convention for the incoming account:
//!
//! 1. No per-account override (the user hasn't pinned this account).
//! 2. No per-institution rule (the maintainer hasn't added the
//!    institution to the YAML registry).
//!
//! In that case, the detector inspects the file itself for evidence:
//!
//! - **Liability accounts (credit cards, loans):** look at rows whose
//!   category is `debt_payment` or whose description matches a payment
//!   keyword (e.g. "PAYMENT - THANK YOU"). Their sign reveals the
//!   convention. A payment row with positive amount means the
//!   institution exports payments as positive credits → the convention
//!   is `PositiveMeansInflow`. Negative payment rows mean
//!   `PositiveMeansOutflow` (Amex/Discover).
//!
//! - **Asset accounts (checking, savings, cash, etc.):** look at
//!   deposit/payroll-like rows (category `income` or descriptions
//!   containing "deposit" / "payroll" / "paycheck"). Their sign
//!   reveals the convention. Positive deposits mean
//!   `PositiveMeansInflow` (the standard for assets).
//!
//! When no signal is present (e.g. a brand-new account with only
//! charges and no payments yet), the detector returns
//! `verdict: None` and the caller falls back to the account-type
//! default.
//!
//! See ADR-018 for the full chain of resolution.

use rust_decimal::Decimal;

use super::sign_normalizer::SignConvention;
use crate::types::AccountType;

/// Minimal row shape the autodetector needs. Built from a parsed
/// transaction or directly from a CSV row by the caller.
#[derive(Debug, Clone)]
pub struct RawRow {
    pub category: Option<String>,
    pub description: String,
    pub amount: Decimal,
}

/// Result of an autodetection attempt.
#[derive(Debug, Clone)]
pub struct AutodetectResult {
    /// `None` when there was insufficient signal to pick a convention.
    pub verdict: Option<SignConvention>,
    /// Fraction of confirming evidence rows: 0.0–1.0. Higher means
    /// stronger consensus among the inspected rows. Callers may apply
    /// a minimum threshold before adopting the verdict.
    pub confidence: f32,
    /// Human-readable explanation, suitable for logging or surfacing
    /// in a post-import banner.
    pub reason: String,
}

/// Substrings that mark a row as a payment receipt on a liability
/// account. Match is case-insensitive on the description text.
const PAYMENT_KEYWORDS: &[&str] = &[
    "payment - thank you",
    "payment thank you",
    "mobile payment",
    "online payment",
    "internet payment",
    "automatic payment",
    "autopay",
    "statement credit",
    "balance transfer",
];

/// Substrings that mark a row as a deposit/paycheck on an asset
/// account. Match is case-insensitive on the description text.
const DEPOSIT_KEYWORDS: &[&str] = &[
    "payroll",
    "direct dep",
    "direct deposit",
    "paycheck",
    "ach credit",
    "deposit",
];

pub struct SignAutodetector;

impl SignAutodetector {
    /// Detect the most likely sign convention for the given account
    /// type from a sample of its transaction rows.
    pub fn detect(account_type: AccountType, rows: &[RawRow]) -> AutodetectResult {
        use crate::types::AccountRole;
        match AccountRole::for_account_type(account_type) {
            AccountRole::Liability => Self::detect_liability(rows),
            AccountRole::Asset => Self::detect_asset(rows),
        }
    }

    fn detect_liability(rows: &[RawRow]) -> AutodetectResult {
        let payments: Vec<&RawRow> = rows.iter().filter(|r| Self::is_payment_row(r)).collect();
        if payments.is_empty() {
            return AutodetectResult {
                verdict: None,
                confidence: 0.0,
                reason: "No payment rows found to infer convention".into(),
            };
        }

        let positive = payments
            .iter()
            .filter(|r| r.amount > Decimal::ZERO)
            .count();
        let negative = payments
            .iter()
            .filter(|r| r.amount < Decimal::ZERO)
            .count();
        let total_signed = positive + negative;
        if total_signed == 0 {
            return AutodetectResult {
                verdict: None,
                confidence: 0.0,
                reason: "All payment rows had zero amount".into(),
            };
        }

        let total = total_signed as f32;
        if positive > negative {
            // Positive payments → exporter sends payments as credits
            // (positive amounts) → convention is PositiveMeansInflow.
            AutodetectResult {
                verdict: Some(SignConvention::PositiveMeansInflow),
                confidence: positive as f32 / total,
                reason: format!(
                    "{} of {} payment rows are positive → PositiveMeansInflow",
                    positive, total_signed
                ),
            }
        } else {
            // Negative payments → exporter sends payments as debits
            // (negative amounts) → convention is PositiveMeansOutflow
            // (Amex/Discover).
            AutodetectResult {
                verdict: Some(SignConvention::PositiveMeansOutflow),
                confidence: negative as f32 / total,
                reason: format!(
                    "{} of {} payment rows are negative → PositiveMeansOutflow",
                    negative, total_signed
                ),
            }
        }
    }

    fn detect_asset(rows: &[RawRow]) -> AutodetectResult {
        let deposits: Vec<&RawRow> = rows.iter().filter(|r| Self::is_deposit_row(r)).collect();
        if deposits.is_empty() {
            return AutodetectResult {
                verdict: None,
                confidence: 0.0,
                reason: "No deposit/payroll rows found".into(),
            };
        }

        let positive = deposits
            .iter()
            .filter(|r| r.amount > Decimal::ZERO)
            .count();
        let negative = deposits
            .iter()
            .filter(|r| r.amount < Decimal::ZERO)
            .count();
        let total_signed = positive + negative;
        if total_signed == 0 {
            return AutodetectResult {
                verdict: None,
                confidence: 0.0,
                reason: "All deposit rows had zero amount".into(),
            };
        }

        let total = total_signed as f32;
        if positive >= negative {
            AutodetectResult {
                verdict: Some(SignConvention::PositiveMeansInflow),
                confidence: positive as f32 / total,
                reason: format!(
                    "{} of {} deposit rows are positive → PositiveMeansInflow",
                    positive, total_signed
                ),
            }
        } else {
            AutodetectResult {
                verdict: Some(SignConvention::PositiveMeansOutflow),
                confidence: negative as f32 / total,
                reason: format!(
                    "{} of {} deposit rows are negative → PositiveMeansOutflow",
                    negative, total_signed
                ),
            }
        }
    }

    fn is_payment_row(r: &RawRow) -> bool {
        if r.category.as_deref() == Some("debt_payment") {
            return true;
        }
        let desc = r.description.to_lowercase();
        PAYMENT_KEYWORDS.iter().any(|kw| desc.contains(kw))
    }

    fn is_deposit_row(r: &RawRow) -> bool {
        if matches!(
            r.category.as_deref(),
            Some("income") | Some("paycheck") | Some("salary") | Some("payroll")
        ) {
            return true;
        }
        let desc = r.description.to_lowercase();
        DEPOSIT_KEYWORDS.iter().any(|kw| desc.contains(kw))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(category: Option<&str>, description: &str, amount_units: i64) -> RawRow {
        RawRow {
            category: category.map(String::from),
            description: description.to_string(),
            amount: Decimal::new(amount_units, 0),
        }
    }

    // ── Liability detection ─────────────────────────────────────────

    #[test]
    fn amex_style_negative_payments_detect_positive_means_outflow() {
        // Amex: charges +, payments −
        let rows = vec![
            r(Some("debt_payment"), "MOBILE PAYMENT - THANK YOU", -240),
            r(Some("food_dining"), "FERN THAI", 28),
            r(Some("travel"), "DELTA AIR LINES", 450),
        ];
        let result = SignAutodetector::detect(AccountType::CreditCard, &rows);
        assert_eq!(
            result.verdict,
            Some(SignConvention::PositiveMeansOutflow),
            "{}",
            result.reason
        );
        assert!(result.confidence >= 0.9, "got {}", result.confidence);
    }

    #[test]
    fn chase_style_positive_payments_detect_positive_means_inflow() {
        // Chase: charges −, payments +
        let rows = vec![
            r(Some("debt_payment"), "INTERNET PAYMENT - THANK YOU", 1500),
            r(Some("food_dining"), "ALBERTSONS", -85),
            r(Some("entertainment"), "NETFLIX", -15),
            r(Some("debt_payment"), "AUTOPAY", 200),
        ];
        let result = SignAutodetector::detect(AccountType::CreditCard, &rows);
        assert_eq!(
            result.verdict,
            Some(SignConvention::PositiveMeansInflow),
            "{}",
            result.reason
        );
        assert!(result.confidence >= 0.9);
    }

    #[test]
    fn no_payment_signals_returns_none() {
        let rows = vec![
            r(Some("food_dining"), "STARBUCKS", 5),
            r(Some("food_dining"), "STARBUCKS", 7),
        ];
        let result = SignAutodetector::detect(AccountType::CreditCard, &rows);
        assert_eq!(result.verdict, None);
        assert_eq!(result.confidence, 0.0);
    }

    #[test]
    fn payment_keyword_matches_without_category() {
        // Sometimes the category is missing but the description is clear.
        let rows = vec![
            r(None, "INTERNET PAYMENT - THANK YOU", 100),
            r(Some("travel"), "UNITED", -50),
        ];
        let result = SignAutodetector::detect(AccountType::CreditCard, &rows);
        assert_eq!(result.verdict, Some(SignConvention::PositiveMeansInflow));
    }

    #[test]
    fn loan_uses_liability_path() {
        // Personal loan with payments imported as positives.
        let rows = vec![r(Some("debt_payment"), "BANK OF AMERICA PAYMENT", 250)];
        let result = SignAutodetector::detect(AccountType::LoanPersonal, &rows);
        assert_eq!(result.verdict, Some(SignConvention::PositiveMeansInflow));
    }

    // ── Asset detection ─────────────────────────────────────────────

    #[test]
    fn checking_with_positive_deposits_detect_positive_means_inflow() {
        let rows = vec![
            r(Some("income"), "ACME CORP PAYROLL", 5000),
            r(Some("food_dining"), "ALBERTSONS", -85),
        ];
        let result = SignAutodetector::detect(AccountType::Checking, &rows);
        assert_eq!(result.verdict, Some(SignConvention::PositiveMeansInflow));
    }

    #[test]
    fn checking_keyword_match_without_category() {
        let rows = vec![r(None, "DIRECT DEPOSIT FROM EMPLOYER", 5000)];
        let result = SignAutodetector::detect(AccountType::Checking, &rows);
        assert_eq!(result.verdict, Some(SignConvention::PositiveMeansInflow));
    }

    #[test]
    fn checking_no_signals_returns_none() {
        let rows = vec![r(Some("food_dining"), "STARBUCKS", -5)];
        let result = SignAutodetector::detect(AccountType::Checking, &rows);
        assert_eq!(result.verdict, None);
    }
}
