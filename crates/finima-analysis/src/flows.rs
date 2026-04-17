//! Inter-account flow detection, Sankey data, outflow ranking, and waterfall.

use std::collections::HashMap;

use chrono::{Datelike, NaiveDate};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::recurring::TransactionForAnalysis;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A candidate inter-account flow detected from transaction matching.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowCandidate {
    pub source_account_id: Uuid,
    pub target_account_id: Option<Uuid>,
    pub source_transaction_id: Uuid,
    pub target_transaction_id: Option<Uuid>,
    pub amount: Decimal,
    pub flow_date: NaiveDate,
    pub is_transfer_like: bool,
}

/// A confirmed/stored flow record (used as input to visualization builders).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowRecord {
    pub source_account_id: Uuid,
    pub source_account_name: String,
    pub target_account_id: Option<Uuid>,
    pub target_account_name: Option<String>,
    pub amount: Decimal,
    pub flow_date: NaiveDate,
    pub category: Option<String>,
}

// --- Sankey ---

/// Complete Sankey diagram data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SankeyData {
    pub nodes: Vec<SankeyNode>,
    pub links: Vec<SankeyLink>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SankeyNode {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SankeyLink {
    pub source: String,
    pub target: String,
    pub value: Decimal,
}

// --- Outflow ranking ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutflowRank {
    pub category: String,
    pub monthly_outflow: Decimal,
    pub percentage_of_income: f64,
}

// --- Waterfall ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaterfallData {
    pub segments: Vec<WaterfallSegment>,
    pub start_balance: Decimal,
    pub end_balance: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaterfallSegment {
    pub label: String,
    pub amount: Decimal,
    /// Running total after this segment.
    pub running_total: Decimal,
}

// ---------------------------------------------------------------------------
// Transfer-like detection
// ---------------------------------------------------------------------------

const TRANSFER_KEYWORDS: &[&str] = &["TRANSFER", "XFER", "AUTOPAY", "PMT", "PAYMENT"];

fn is_transfer_like(description: &str) -> bool {
    let upper = description.to_uppercase();
    TRANSFER_KEYWORDS.iter().any(|kw| upper.contains(kw))
}

// ---------------------------------------------------------------------------
// Flow detection
// ---------------------------------------------------------------------------

/// Detect inter-account flows by matching outflows from primary accounts to
/// inflows in other accounts.
///
/// Matching criteria:
/// - Amount within +/-1%
/// - Date within +/-2 days
pub fn detect_flows(
    primary_accounts: &[Uuid],
    transactions_by_account: &HashMap<Uuid, Vec<TransactionForAnalysis>>,
) -> Vec<FlowCandidate> {
    let mut candidates = Vec::new();
    let primary_set: std::collections::HashSet<Uuid> = primary_accounts.iter().copied().collect();

    // Collect all inflows from non-primary accounts for matching.
    let mut all_inflows: Vec<(&Uuid, &TransactionForAnalysis)> = Vec::new();
    for (acct_id, txns) in transactions_by_account {
        for txn in txns {
            if txn.amount > Decimal::ZERO {
                all_inflows.push((acct_id, txn));
            }
        }
    }

    // Track used inflow transaction IDs to prevent double-matching.
    let mut used_inflows: std::collections::HashSet<Uuid> = std::collections::HashSet::new();

    for primary_id in &primary_set {
        let Some(txns) = transactions_by_account.get(primary_id) else {
            continue;
        };

        for txn in txns {
            // Only consider outflows.
            if txn.amount >= Decimal::ZERO {
                continue;
            }
            let outflow_abs = txn.amount.abs();

            // Try to find a matching inflow.
            let mut matched = false;
            for (inflow_acct, inflow_txn) in &all_inflows {
                if **inflow_acct == *primary_id {
                    continue; // skip same account
                }
                if used_inflows.contains(&inflow_txn.id) {
                    continue;
                }

                // Amount within 1%
                let tolerance = outflow_abs * Decimal::new(1, 2); // 1%
                let amount_diff = (inflow_txn.amount - outflow_abs).abs();
                if amount_diff > tolerance {
                    continue;
                }

                // Date within 2 days
                let day_diff = (inflow_txn.date - txn.date).num_days().abs();
                if day_diff > 2 {
                    continue;
                }

                used_inflows.insert(inflow_txn.id);
                candidates.push(FlowCandidate {
                    source_account_id: *primary_id,
                    target_account_id: Some(**inflow_acct),
                    source_transaction_id: txn.id,
                    target_transaction_id: Some(inflow_txn.id),
                    amount: outflow_abs,
                    flow_date: txn.date,
                    is_transfer_like: true,
                });
                matched = true;
                break;
            }

            // If no match found but description looks transfer-like, create one-sided.
            if !matched && is_transfer_like(&txn.description) {
                candidates.push(FlowCandidate {
                    source_account_id: *primary_id,
                    target_account_id: None,
                    source_transaction_id: txn.id,
                    target_transaction_id: None,
                    amount: outflow_abs,
                    flow_date: txn.date,
                    is_transfer_like: true,
                });
            }
        }
    }

    candidates
}

// ---------------------------------------------------------------------------
// Sankey
// ---------------------------------------------------------------------------

/// Build Sankey diagram data from flow records for a given month.
pub fn build_sankey_data(flows: &[FlowRecord], month: NaiveDate) -> SankeyData {
    let target_year = month.year();
    let target_month = month.month();

    // Aggregate flows by (source_name, target_name).
    let mut link_map: HashMap<(String, String), Decimal> = HashMap::new();
    let mut node_set: std::collections::HashSet<String> = std::collections::HashSet::new();

    for flow in flows {
        if flow.flow_date.year() != target_year || flow.flow_date.month() != target_month {
            continue;
        }
        let target_name = flow
            .target_account_name
            .clone()
            .or_else(|| flow.category.clone())
            .unwrap_or_else(|| "Unknown".to_string());

        node_set.insert(flow.source_account_name.clone());
        node_set.insert(target_name.clone());

        *link_map
            .entry((flow.source_account_name.clone(), target_name))
            .or_default() += flow.amount;
    }

    let nodes: Vec<SankeyNode> = node_set
        .into_iter()
        .map(|name| SankeyNode {
            id: name.clone(),
            label: name,
        })
        .collect();

    let links: Vec<SankeyLink> = link_map
        .into_iter()
        .map(|((source, target), value)| SankeyLink {
            source,
            target,
            value,
        })
        .collect();

    SankeyData { nodes, links }
}

// ---------------------------------------------------------------------------
// Outflow ranking
// ---------------------------------------------------------------------------

/// Build an outflow ranking sorted by monthly outflow descending.
pub fn build_outflow_ranking(flows: &[FlowRecord], total_income: Decimal) -> Vec<OutflowRank> {
    // Group by category.
    let mut by_category: HashMap<String, Decimal> = HashMap::new();
    for flow in flows {
        let cat = flow
            .category
            .clone()
            .unwrap_or_else(|| "Uncategorized".to_string());
        *by_category.entry(cat).or_default() += flow.amount;
    }

    let income_f = total_income.to_f64().unwrap_or(1.0);

    let mut ranking: Vec<OutflowRank> = by_category
        .into_iter()
        .map(|(category, monthly_outflow)| {
            let pct = if income_f > 0.0 {
                monthly_outflow.to_f64().unwrap_or(0.0) / income_f * 100.0
            } else {
                0.0
            };
            OutflowRank {
                category,
                monthly_outflow,
                percentage_of_income: pct,
            }
        })
        .collect();

    ranking.sort_by_key(|r| std::cmp::Reverse(r.monthly_outflow));
    ranking
}

// ---------------------------------------------------------------------------
// Waterfall
// ---------------------------------------------------------------------------

/// Build a waterfall chart: start + income - outflows = end.
pub fn build_waterfall(
    _account_id: Uuid,
    start_balance: Decimal,
    income: Decimal,
    outflows: &[(String, Decimal)],
) -> WaterfallData {
    let mut segments = Vec::new();
    let mut running = start_balance;

    // Start segment.
    segments.push(WaterfallSegment {
        label: "Starting Balance".to_string(),
        amount: start_balance,
        running_total: running,
    });

    // Income segment.
    running += income;
    segments.push(WaterfallSegment {
        label: "Income".to_string(),
        amount: income,
        running_total: running,
    });

    // Outflow segments.
    for (label, amount) in outflows {
        running -= *amount;
        segments.push(WaterfallSegment {
            label: label.clone(),
            amount: -*amount,
            running_total: running,
        });
    }

    WaterfallData {
        segments,
        start_balance,
        end_balance: running,
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

    fn make_txn(
        id: u128,
        date: &str,
        amount: Decimal,
        desc: &str,
        acct: Option<u128>,
    ) -> TransactionForAnalysis {
        TransactionForAnalysis {
            id: Uuid::from_u128(id),
            date: NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap(),
            amount,
            description: desc.to_string(),
            merchant_name: None,
            category: None,
            account_id: acct.map(Uuid::from_u128),
        }
    }

    // --- Flow detection tests ---

    #[test]
    fn matching_pair_found() {
        let primary = vec![Uuid::from_u128(1)];
        let mut by_acct = HashMap::new();
        by_acct.insert(
            Uuid::from_u128(1),
            vec![make_txn(
                10,
                "2025-03-01",
                dec!(-500),
                "Transfer out",
                Some(1),
            )],
        );
        by_acct.insert(
            Uuid::from_u128(2),
            vec![make_txn(
                20,
                "2025-03-01",
                dec!(500),
                "Transfer in",
                Some(2),
            )],
        );

        let flows = detect_flows(&primary, &by_acct);
        assert_eq!(flows.len(), 1);
        assert!(flows[0].target_transaction_id.is_some());
        assert_eq!(flows[0].amount, dec!(500));
    }

    #[test]
    fn no_match_amount_diff_over_1_percent() {
        let primary = vec![Uuid::from_u128(1)];
        let mut by_acct = HashMap::new();
        by_acct.insert(
            Uuid::from_u128(1),
            vec![make_txn(10, "2025-03-01", dec!(-500), "Out", Some(1))],
        );
        by_acct.insert(
            Uuid::from_u128(2),
            // 510 is >1% of 500 = 505 threshold
            vec![make_txn(20, "2025-03-01", dec!(510), "In", Some(2))],
        );

        let flows = detect_flows(&primary, &by_acct);
        // Should not match (amount diff 10 > 5 = 1% of 500)
        assert!(flows.is_empty());
    }

    #[test]
    fn no_match_date_diff_over_2_days() {
        let primary = vec![Uuid::from_u128(1)];
        let mut by_acct = HashMap::new();
        by_acct.insert(
            Uuid::from_u128(1),
            vec![make_txn(10, "2025-03-01", dec!(-500), "Out", Some(1))],
        );
        by_acct.insert(
            Uuid::from_u128(2),
            vec![make_txn(20, "2025-03-05", dec!(500), "In", Some(2))],
        );

        let flows = detect_flows(&primary, &by_acct);
        assert!(flows.is_empty());
    }

    #[test]
    fn one_sided_flow_transfer_keyword() {
        let primary = vec![Uuid::from_u128(1)];
        let mut by_acct = HashMap::new();
        by_acct.insert(
            Uuid::from_u128(1),
            vec![make_txn(
                10,
                "2025-03-01",
                dec!(-300),
                "AUTOPAY CREDIT CARD",
                Some(1),
            )],
        );

        let flows = detect_flows(&primary, &by_acct);
        assert_eq!(flows.len(), 1);
        assert!(flows[0].target_transaction_id.is_none());
        assert!(flows[0].is_transfer_like);
    }

    #[test]
    fn non_transfer_outflow_ignored() {
        let primary = vec![Uuid::from_u128(1)];
        let mut by_acct = HashMap::new();
        by_acct.insert(
            Uuid::from_u128(1),
            vec![make_txn(
                10,
                "2025-03-01",
                dec!(-50),
                "Coffee Shop",
                Some(1),
            )],
        );

        let flows = detect_flows(&primary, &by_acct);
        assert!(flows.is_empty());
    }

    #[test]
    fn transfer_keyword_detection() {
        assert!(is_transfer_like("ACH TRANSFER 12345"));
        assert!(is_transfer_like("XFER TO SAVINGS"));
        assert!(is_transfer_like("AUTOPAY MORTGAGE"));
        assert!(is_transfer_like("PMT FROM CHECKING"));
        assert!(is_transfer_like("CC PAYMENT"));
        assert!(!is_transfer_like("STARBUCKS COFFEE"));
    }

    // --- Sankey tests ---

    #[test]
    fn sankey_aggregation() {
        let flows = vec![
            FlowRecord {
                source_account_id: Uuid::from_u128(1),
                source_account_name: "Checking".into(),
                target_account_id: Some(Uuid::from_u128(2)),
                target_account_name: Some("Savings".into()),
                amount: dec!(500),
                flow_date: NaiveDate::from_ymd_opt(2025, 3, 5).unwrap(),
                category: None,
            },
            FlowRecord {
                source_account_id: Uuid::from_u128(1),
                source_account_name: "Checking".into(),
                target_account_id: Some(Uuid::from_u128(2)),
                target_account_name: Some("Savings".into()),
                amount: dec!(300),
                flow_date: NaiveDate::from_ymd_opt(2025, 3, 15).unwrap(),
                category: None,
            },
        ];

        let data = build_sankey_data(&flows, NaiveDate::from_ymd_opt(2025, 3, 1).unwrap());
        assert_eq!(data.nodes.len(), 2);
        assert_eq!(data.links.len(), 1);
        assert_eq!(data.links[0].value, dec!(800));
    }

    // --- Outflow ranking tests ---

    #[test]
    fn outflow_ranking_sorted_and_percentages() {
        let flows = vec![
            FlowRecord {
                source_account_id: Uuid::from_u128(1),
                source_account_name: "Checking".into(),
                target_account_id: None,
                target_account_name: None,
                amount: dec!(200),
                flow_date: NaiveDate::from_ymd_opt(2025, 3, 1).unwrap(),
                category: Some("Groceries".into()),
            },
            FlowRecord {
                source_account_id: Uuid::from_u128(1),
                source_account_name: "Checking".into(),
                target_account_id: None,
                target_account_name: None,
                amount: dec!(800),
                flow_date: NaiveDate::from_ymd_opt(2025, 3, 1).unwrap(),
                category: Some("Rent".into()),
            },
        ];

        let ranking = build_outflow_ranking(&flows, dec!(5000));
        assert_eq!(ranking.len(), 2);
        assert_eq!(ranking[0].category, "Rent");
        assert_eq!(ranking[0].monthly_outflow, dec!(800));
        assert!((ranking[0].percentage_of_income - 16.0).abs() < 0.01);
        assert_eq!(ranking[1].category, "Groceries");
    }

    // --- Waterfall tests ---

    #[test]
    fn waterfall_arithmetic() {
        let outflows = vec![
            ("Rent".to_string(), dec!(1500)),
            ("Groceries".to_string(), dec!(400)),
        ];
        let data = build_waterfall(Uuid::from_u128(1), dec!(5000), dec!(6000), &outflows);

        // start(5000) + income(6000) - rent(1500) - groceries(400) = 9100
        assert_eq!(data.end_balance, dec!(9100));
        assert_eq!(data.segments.len(), 4); // start + income + 2 outflows

        // Verify running totals.
        assert_eq!(data.segments[0].running_total, dec!(5000));
        assert_eq!(data.segments[1].running_total, dec!(11000));
        assert_eq!(data.segments[2].running_total, dec!(9500));
        assert_eq!(data.segments[3].running_total, dec!(9100));
    }

    #[test]
    fn waterfall_no_outflows() {
        let data = build_waterfall(Uuid::from_u128(1), dec!(1000), dec!(2000), &[]);
        assert_eq!(data.end_balance, dec!(3000));
        assert_eq!(data.segments.len(), 2);
    }
}
