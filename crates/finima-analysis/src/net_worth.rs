//! Net worth time series computation.

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use finima_core::types::{AccountRole, AccountType};
use finima_core::{next_month_start, start_of_month};

use crate::recurring::TransactionForAnalysis;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A snapshot of an account at the start of analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountSnapshot {
    pub id: Uuid,
    pub opening_balance: Decimal,
    pub account_type: AccountType,
    pub is_archived: bool,
}

/// A single point in the net worth time series.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetWorthPoint {
    pub date: NaiveDate,
    pub total: Decimal,
    pub assets: Decimal,
    pub liabilities: Decimal,
}

// ---------------------------------------------------------------------------
// Functions
// ---------------------------------------------------------------------------

/// Compute a net worth time series from `start` to `end` (first-of-month
/// for efficiency).
///
/// For each first-of-month in the range, the balance of each account is:
///   opening_balance + sum(transactions before that date)
///
/// Credit cards and loans are treated as liabilities (negative net worth).
pub fn compute_net_worth_series(
    accounts: &[AccountSnapshot],
    transactions: &[TransactionForAnalysis],
    start: NaiveDate,
    end: NaiveDate,
) -> Vec<NetWorthPoint> {
    // Build list of first-of-month dates in range.
    let mut dates = Vec::new();
    let mut current = start_of_month(start);
    while current <= end {
        dates.push(current);
        current = next_month_start(current);
    }

    // Pre-sort transactions per account.
    let mut txns_by_account: std::collections::HashMap<Uuid, Vec<&TransactionForAnalysis>> =
        std::collections::HashMap::new();
    for txn in transactions {
        if let Some(acct_id) = txn.account_id {
            txns_by_account.entry(acct_id).or_default().push(txn);
        }
    }

    dates
        .into_iter()
        .map(|date| {
            let mut assets = Decimal::ZERO;
            let mut liabilities = Decimal::ZERO;

            for acct in accounts {
                if acct.is_archived {
                    continue;
                }
                // Balance = opening + sum(txns before date)
                let txn_sum: Decimal = txns_by_account
                    .get(&acct.id)
                    .map(|txns| {
                        txns.iter()
                            .filter(|t| t.date < date)
                            .map(|t| t.amount)
                            .sum()
                    })
                    .unwrap_or_default();

                let balance = acct.opening_balance + txn_sum;

                // Canonical amounts (ADR-018): split the signed
                // balance into (asset, liability) contributions in
                // one place. See AccountRole::classify_balance.
                let (a, l) = AccountRole::classify_balance(acct.account_type, balance);
                assets += a;
                liabilities += l;
            }

            NetWorthPoint {
                date,
                total: assets - liabilities,
                assets,
                liabilities,
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use rust_decimal_macros::dec;

    fn acct(id: u128, balance: Decimal, at: AccountType) -> AccountSnapshot {
        AccountSnapshot {
            id: Uuid::from_u128(id),
            opening_balance: balance,
            account_type: at,
            is_archived: false,
        }
    }

    fn txn(acct_id: u128, date: &str, amount: Decimal) -> TransactionForAnalysis {
        TransactionForAnalysis {
            id: Uuid::new_v4(),
            date: NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap(),
            amount,
            description: "test".into(),
            merchant_name: None,
            category: None,
            account_id: Some(Uuid::from_u128(acct_id)),
        }
    }

    #[test]
    fn simple_net_worth() {
        let accounts = vec![acct(1, dec!(10000), AccountType::Checking)];
        let txns = vec![
            txn(1, "2025-01-15", dec!(-500)),
            txn(1, "2025-02-10", dec!(3000)),
        ];
        let result = compute_net_worth_series(
            &accounts,
            &txns,
            NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(2025, 3, 1).unwrap(),
        );
        assert_eq!(result.len(), 3); // Jan, Feb, Mar
                                     // Jan 1: opening only (no txns before Jan 1)
        assert_eq!(result[0].assets, dec!(10000));
        // Feb 1: opening + Jan txn (-500)
        assert_eq!(result[1].assets, dec!(9500));
        // Mar 1: opening + Jan txn + Feb txn
        assert_eq!(result[2].assets, dec!(12500));
    }

    #[test]
    fn credit_card_debt_counts_as_liability() {
        // Canonical balances (ADR-018): a credit card with outstanding
        // debt has a *negative* balance. A -2000 balance represents
        // $2000 owed to the card issuer.
        let accounts = vec![
            acct(1, dec!(5000), AccountType::Checking),
            acct(2, dec!(-2000), AccountType::CreditCard),
        ];
        let result = compute_net_worth_series(
            &accounts,
            &[],
            NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
        );
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].assets, dec!(5000));
        assert_eq!(result[0].liabilities, dec!(2000));
        assert_eq!(result[0].total, dec!(3000));
    }

    #[test]
    fn credit_card_positive_balance_counts_as_asset() {
        // A positive liability balance = user overpaid the card and
        // has a credit sitting on it — treat as usable cash, not
        // debt. No value should end up in `liabilities`.
        let accounts = vec![
            acct(1, dec!(1000), AccountType::Checking),
            acct(2, dec!(250), AccountType::CreditCard),
        ];
        let result = compute_net_worth_series(
            &accounts,
            &[],
            NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
        );
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].assets, dec!(1250));
        assert_eq!(result[0].liabilities, dec!(0));
        assert_eq!(result[0].total, dec!(1250));
    }

    #[test]
    fn time_series_across_months() {
        let accounts = vec![acct(1, dec!(1000), AccountType::Savings)];
        let txns = vec![
            txn(1, "2025-01-15", dec!(100)),
            txn(1, "2025-02-15", dec!(100)),
            txn(1, "2025-03-15", dec!(100)),
        ];
        let result = compute_net_worth_series(
            &accounts,
            &txns,
            NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(2025, 4, 1).unwrap(),
        );
        assert_eq!(result.len(), 4);
        assert_eq!(result[0].assets, dec!(1000)); // Jan 1
        assert_eq!(result[1].assets, dec!(1100)); // Feb 1
        assert_eq!(result[2].assets, dec!(1200)); // Mar 1
        assert_eq!(result[3].assets, dec!(1300)); // Apr 1
    }

    #[test]
    fn archived_account_excluded() {
        let accounts = vec![AccountSnapshot {
            id: Uuid::from_u128(1),
            opening_balance: dec!(10000),
            account_type: AccountType::Checking,
            is_archived: true,
        }];
        let result = compute_net_worth_series(
            &accounts,
            &[],
            NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
        );
        assert_eq!(result[0].total, dec!(0));
    }
}
