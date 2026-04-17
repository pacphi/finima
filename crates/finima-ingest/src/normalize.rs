//! Direction normalization for parsed batches.
//!
//! Bridges the institution-agnostic [`RawTransaction`] produced by the
//! file parsers to the canonical [`TransactionDirection`] expected on
//! every persisted row. Consults the configured [`SignNormalizer`]
//! plus a one-shot [`SignAutodetector`] pass to handle institutions
//! the maintainer hasn't yet curated.
//!
//! See ADR-018 for the full resolution chain.

use finima_core::services::sign_autodetector::{AutodetectResult, RawRow, SignAutodetector};
use finima_core::services::sign_normalizer::{AccountContext, SignNormalizer};
use finima_core::types::{AccountType, TransactionDirection};
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::RawTransaction;

/// Outcome of [`normalize_batch`].
#[derive(Debug, Clone)]
pub struct NormalizationResult {
    /// Direction for each input row, aligned by index.
    pub directions: Vec<TransactionDirection>,
    /// Canonical (positive_means_inflow) amount for each input row,
    /// aligned by index. This is what the import pipeline persists
    /// to `transactions.amount`.
    ///
    /// Post-normalization invariant (for non-zero rows):
    /// `direction == Inflow  <=> amounts[i] > 0`
    /// `direction == Outflow <=> amounts[i] < 0`
    pub amounts: Vec<Decimal>,
    /// Autodetection result, when run. `None` when the normalizer
    /// resolved a convention from per-account override or
    /// per-institution rule (no detection needed).
    pub autodetection: Option<AutodetectResult>,
}

/// Compute [`TransactionDirection`] for every row in `raw` by
/// consulting `normalizer`, falling back to autodetection from the
/// rows themselves when no per-institution rule matches.
///
/// The pipeline is:
///
/// 1. Decide whether autodetection is needed: only when neither the
///    per-account override nor the per-institution rule is present.
/// 2. If needed, run [`SignAutodetector::detect`] against the rows
///    and slot the verdict into the normalizer's
///    `direction_for_with_detection` chain.
/// 3. Apply the normalizer to each row.
pub fn normalize_batch(
    raw: &[RawTransaction],
    account_id: Uuid,
    account_type: AccountType,
    institution: Option<&str>,
    normalizer: &SignNormalizer,
) -> NormalizationResult {
    let needs_detection = !normalizer.rules.by_account_id.contains_key(&account_id)
        && institution
            .map(|inst| {
                !normalizer
                    .rules
                    .by_institution
                    .contains_key(&inst.to_lowercase())
            })
            .unwrap_or(true);

    let autodetection = if needs_detection {
        let rows: Vec<RawRow> = raw
            .iter()
            .map(|t| RawRow {
                category: t.category.clone(),
                description: t.description.clone(),
                amount: t.amount,
            })
            .collect();
        Some(SignAutodetector::detect(account_type, &rows))
    } else {
        None
    };
    let detected_convention = autodetection.as_ref().and_then(|d| d.verdict);

    let ctx = AccountContext {
        account_id,
        account_type,
        institution: institution.map(str::to_owned),
    };

    let mut directions: Vec<TransactionDirection> = Vec::with_capacity(raw.len());
    let mut amounts: Vec<Decimal> = Vec::with_capacity(raw.len());
    for t in raw {
        let (dir, canonical) =
            normalizer.normalize_with_detection(&ctx, t.amount, detected_convention);
        directions.push(dir);
        amounts.push(canonical);
    }

    NormalizationResult {
        directions,
        amounts,
        autodetection,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use finima_core::services::sign_normalizer::{SignConvention, SignConventions};
    use rust_decimal::Decimal;

    fn raw(category: Option<&str>, description: &str, amount_units: i64) -> RawTransaction {
        RawTransaction {
            date: NaiveDate::from_ymd_opt(2026, 3, 14).unwrap(),
            amount: Decimal::new(amount_units, 0),
            description: description.to_string(),
            original_description: description.to_string(),
            memo: None,
            category: category.map(String::from),
        }
    }

    #[test]
    fn institution_rule_skips_autodetection() {
        let mut rules = SignConventions::default();
        rules.set_institution("chase", SignConvention::PositiveMeansInflow);
        let normalizer = SignNormalizer::new(rules);

        let rows = vec![
            raw(Some("debt_payment"), "PAYMENT - THANK YOU", 1500),
            raw(Some("food_dining"), "ALBERTSONS", -85),
        ];
        let result = normalize_batch(
            &rows,
            Uuid::new_v4(),
            AccountType::CreditCard,
            Some("chase"),
            &normalizer,
        );

        assert!(
            result.autodetection.is_none(),
            "should not run autodetection when institution rule exists"
        );
        // Chase: positive = inflow, negative = outflow.
        assert_eq!(result.directions[0], TransactionDirection::Inflow);
        assert_eq!(result.directions[1], TransactionDirection::Outflow);
        // Chase is already in the canonical convention, so amounts pass
        // through unchanged.
        assert_eq!(result.amounts[0], Decimal::new(1500, 0));
        assert_eq!(result.amounts[1], Decimal::new(-85, 0));
    }

    #[test]
    fn unknown_institution_runs_autodetection_with_chase_signature() {
        let normalizer = SignNormalizer::new(SignConventions::default());
        let rows = vec![
            raw(Some("debt_payment"), "INTERNET PAYMENT - THANK YOU", 1500),
            raw(Some("food_dining"), "STARBUCKS", -10),
        ];
        let result = normalize_batch(
            &rows,
            Uuid::new_v4(),
            AccountType::CreditCard,
            Some("smaller_credit_union"),
            &normalizer,
        );

        let autodetection = result.autodetection.expect("should run autodetection");
        assert_eq!(
            autodetection.verdict,
            Some(SignConvention::PositiveMeansInflow)
        );
        // Negative -> outflow under detected convention.
        assert_eq!(result.directions[1], TransactionDirection::Outflow);
    }

    #[test]
    fn unknown_institution_runs_autodetection_with_amex_signature() {
        let normalizer = SignNormalizer::new(SignConventions::default());
        let rows = vec![
            raw(Some("debt_payment"), "MOBILE PAYMENT - THANK YOU", -240),
            raw(Some("food_dining"), "FERN THAI", 28),
        ];
        let result = normalize_batch(
            &rows,
            Uuid::new_v4(),
            AccountType::CreditCard,
            Some("regional_bank"),
            &normalizer,
        );

        let autodetection = result.autodetection.expect("should run autodetection");
        assert_eq!(
            autodetection.verdict,
            Some(SignConvention::PositiveMeansOutflow)
        );
        // Positive -> outflow under detected (Amex) convention.
        assert_eq!(result.directions[1], TransactionDirection::Outflow);
        // Amex-signature amounts flip to canonical (positive_means_inflow):
        // raw -240 payment becomes +240 (inflow), raw +28 charge becomes -28.
        assert_eq!(result.amounts[0], Decimal::new(240, 0));
        assert_eq!(result.amounts[1], Decimal::new(-28, 0));
    }

    #[test]
    fn no_signal_falls_back_to_account_type_default() {
        let normalizer = SignNormalizer::new(SignConventions::default());
        // Credit card with only charges, no payment rows -> autodetection
        // returns None verdict -> fall back to account-type default
        // (PositiveMeansOutflow for credit cards).
        let rows = vec![
            raw(Some("food_dining"), "STARBUCKS", 5),
            raw(Some("food_dining"), "STARBUCKS", 7),
        ];
        let result = normalize_batch(
            &rows,
            Uuid::new_v4(),
            AccountType::CreditCard,
            Some("regional_bank"),
            &normalizer,
        );

        let autodetection = result.autodetection.expect("detection still runs");
        assert_eq!(autodetection.verdict, None);
        // Default for credit card: positive = outflow.
        assert_eq!(result.directions[0], TransactionDirection::Outflow);
        assert_eq!(result.directions[1], TransactionDirection::Outflow);
    }

    #[test]
    fn checking_account_with_payroll_detects_inflow_default() {
        let normalizer = SignNormalizer::new(SignConventions::default());
        let rows = vec![
            raw(Some("income"), "ACME CORP PAYROLL", 5000),
            raw(Some("food_dining"), "ALBERTSONS", -85),
        ];
        let result = normalize_batch(
            &rows,
            Uuid::new_v4(),
            AccountType::Checking,
            Some("any_bank"),
            &normalizer,
        );

        // Detection runs, confirms PositiveMeansInflow (also the default).
        let autodetection = result.autodetection.expect("detection runs");
        assert_eq!(
            autodetection.verdict,
            Some(SignConvention::PositiveMeansInflow)
        );
        assert_eq!(result.directions[0], TransactionDirection::Inflow);
        assert_eq!(result.directions[1], TransactionDirection::Outflow);
    }
}
