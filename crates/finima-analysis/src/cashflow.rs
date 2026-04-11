//! Cash flow aggregation by month.

use std::collections::BTreeMap;

use chrono::{Datelike, NaiveDate};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::recurring::TransactionForAnalysis;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Monthly cash flow summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonthlyCashFlow {
    /// First day of the month (e.g. 2025-03-01).
    pub month: NaiveDate,
    pub income: Decimal,
    pub expenses: Decimal,
    pub net: Decimal,
}

// ---------------------------------------------------------------------------
// Functions
// ---------------------------------------------------------------------------

/// Compute monthly cash flow from transactions over the last `months` months.
///
/// Returns entries sorted by month ascending.  If `months` is 0, all
/// transactions are considered.
pub fn compute_monthly_cashflow(
    transactions: &[TransactionForAnalysis],
    months: usize,
) -> Vec<MonthlyCashFlow> {
    // Determine cutoff date if months > 0.
    let cutoff = if months > 0 {
        let now = transactions.iter().map(|t| t.date).max();
        now.and_then(|d| {
            // Go back `months` months from the latest transaction.
            let target_month = if d.month() as i32 - months as i32 > 0 {
                d.month() - months as u32
            } else {
                0
            };
            if target_month > 0 {
                NaiveDate::from_ymd_opt(d.year(), target_month, 1)
            } else {
                let years_back = ((months as i32 - d.month() as i32) / 12) + 1;
                let remaining = months as i32 - d.month() as i32 - (years_back - 1) * 12;
                let m = 12 - remaining as u32;
                NaiveDate::from_ymd_opt(d.year() - years_back, m, 1)
            }
        })
    } else {
        None
    };

    let mut monthly: BTreeMap<(i32, u32), (Decimal, Decimal)> = BTreeMap::new();

    for txn in transactions {
        if let Some(c) = cutoff {
            if txn.date < c {
                continue;
            }
        }
        let key = (txn.date.year(), txn.date.month());
        let entry = monthly.entry(key).or_default();
        if txn.amount > Decimal::ZERO {
            entry.0 += txn.amount; // income
        } else {
            entry.1 += txn.amount.abs(); // expenses (store positive)
        }
    }

    monthly
        .into_iter()
        .map(|((year, month), (income, expenses))| {
            let date = NaiveDate::from_ymd_opt(year, month, 1).unwrap();
            MonthlyCashFlow {
                month: date,
                income,
                expenses,
                net: income - expenses,
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
    use uuid::Uuid;

    fn txn(date: &str, amount: Decimal) -> TransactionForAnalysis {
        TransactionForAnalysis {
            id: Uuid::new_v4(),
            date: NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap(),
            amount,
            description: "test".into(),
            merchant_name: None,
            category: None,
            account_id: None,
        }
    }

    #[test]
    fn separates_income_and_expenses() {
        let txns = vec![
            txn("2025-03-01", dec!(5000)),  // income
            txn("2025-03-15", dec!(-2000)), // expense
            txn("2025-03-20", dec!(-500)),  // expense
        ];
        let result = compute_monthly_cashflow(&txns, 0);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].income, dec!(5000));
        assert_eq!(result[0].expenses, dec!(2500));
        assert_eq!(result[0].net, dec!(2500));
    }

    #[test]
    fn multiple_months_sorted() {
        let txns = vec![
            txn("2025-03-01", dec!(5000)),
            txn("2025-01-15", dec!(4500)),
            txn("2025-02-10", dec!(-1000)),
        ];
        let result = compute_monthly_cashflow(&txns, 0);
        assert_eq!(result.len(), 3);
        assert!(result[0].month < result[1].month);
        assert!(result[1].month < result[2].month);
    }

    #[test]
    fn empty_transactions() {
        let result = compute_monthly_cashflow(&[], 0);
        assert!(result.is_empty());
    }

    #[test]
    fn month_with_only_expenses() {
        let txns = vec![txn("2025-03-05", dec!(-100)), txn("2025-03-10", dec!(-200))];
        let result = compute_monthly_cashflow(&txns, 0);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].income, dec!(0));
        assert_eq!(result[0].expenses, dec!(300));
        assert_eq!(result[0].net, dec!(-300));
    }
}
