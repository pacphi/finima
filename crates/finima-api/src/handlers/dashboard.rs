use axum::extract::{Query, State};
use axum::response::IntoResponse;
use axum::Json;
use chrono::{Months, NaiveDate, Utc};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use finima_analysis::{
    compute_health_score, compute_monthly_cashflow, compute_net_worth_series, AccountSnapshot,
    HealthScoreInput,
};
use finima_auth::middleware::AuthUser;
use finima_core::traits::AccountRepo;
use finima_core::types::{AccountRole, AccountType};
use finima_core::{next_month_start, AppError};

use crate::state::AppState;

use super::helpers::{first_portfolio_id, parse_month, to_analysis};

// ---------------------------------------------------------------------------
// Request / response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct MonthsQuery {
    #[serde(default = "default_months")]
    pub months: usize,
}

fn default_months() -> usize {
    12
}

#[derive(Debug, Deserialize)]
pub struct MonthQuery {
    /// Month in YYYY-MM format; defaults to current month.
    pub month: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DashboardSummary {
    pub net_worth: Decimal,
    pub total_assets: Decimal,
    pub total_liabilities: Decimal,
    pub account_count: usize,
    pub monthly_income: Decimal,
    pub monthly_expenses: Decimal,
    pub savings_rate: f64,
    pub health_score: u8,
    pub upcoming_bills_count: i64,
}

#[derive(Debug, Serialize)]
pub struct NetWorthEntry {
    pub date: NaiveDate,
    pub total: Decimal,
    pub assets: Decimal,
    pub liabilities: Decimal,
}

#[derive(Debug, Serialize)]
pub struct CashFlowEntry {
    pub month: NaiveDate,
    pub income: Decimal,
    pub expenses: Decimal,
    pub net: Decimal,
}

#[derive(Debug, Serialize)]
pub struct SpendingEntry {
    pub category: String,
    pub amount: Decimal,
    pub percentage: f64,
}

#[derive(Debug, Serialize)]
pub struct SubcategorySpendEntry {
    pub subcategory: String,
    pub amount: Decimal,
    pub percentage: f64,
}

#[derive(Debug, Deserialize)]
pub struct SubcategorySpendQuery {
    pub category: String,
    pub month: Option<String>,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// GET /api/dashboard/summary
pub async fn get_summary(
    user: AuthUser,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let portfolio_id = first_portfolio_id(&user, &state).await?;

    // Fetch all accounts and transactions for analysis.
    let accounts = state.account_repo().list_by_portfolio(portfolio_id).await?;
    let txn_rows = state
        .transaction_repo()
        .list_for_analysis(portfolio_id, None, None)
        .await?;
    let txns = to_analysis(&txn_rows);

    // Compute net worth from today's actual account balances. The
    // canonical-amount split lives in AccountRole::classify_balance
    // (ADR-018) so every handler, CLI, and analysis helper agrees on
    // what "debt" means.
    let mut total_assets = Decimal::ZERO;
    let mut total_liabilities = Decimal::ZERO;
    let mut liquid_savings = Decimal::ZERO;
    for acct in &accounts {
        if acct.is_archived {
            continue;
        }
        let balance = state.account_repo().compute_balance(acct.id).await?;
        let (a, l) = AccountRole::classify_balance(acct.account_type, balance);
        total_assets += a;
        total_liabilities += l;
        if matches!(
            acct.account_type,
            AccountType::Checking | AccountType::Savings | AccountType::Cash
        ) {
            liquid_savings += balance;
        }
    }
    let net_worth = total_assets - total_liabilities;

    // Current month cash flow.
    let cashflow = compute_monthly_cashflow(&txns, 1);
    let monthly_income = cashflow.last().map(|c| c.income).unwrap_or_default();
    let monthly_expenses = cashflow.last().map(|c| c.expenses).unwrap_or_default();

    // Average monthly expenses (last 3 months).
    let cashflow_3 = compute_monthly_cashflow(&txns, 3);
    let avg_expenses = if cashflow_3.is_empty() {
        Decimal::ZERO
    } else {
        let total: Decimal = cashflow_3.iter().map(|c| c.expenses).sum();
        total / Decimal::from(cashflow_3.len() as i64)
    };

    let last_month_expenses = cashflow.last().map(|c| c.expenses).unwrap_or_default();

    let health = compute_health_score(&HealthScoreInput {
        monthly_income,
        monthly_expenses,
        total_assets,
        total_liabilities,
        liquid_savings,
        avg_monthly_expenses: avg_expenses,
        last_month_expenses,
    });

    // Upcoming bills: count confirmed recurring groups.
    let recurring_groups = state
        .recurring_repo()
        .list_by_portfolio(portfolio_id)
        .await?;
    let upcoming_bills_count = recurring_groups.iter().filter(|g| g.is_confirmed).count() as i64;

    let account_count = accounts.iter().filter(|a| !a.is_archived).count();

    Ok(Json(DashboardSummary {
        net_worth,
        total_assets,
        total_liabilities,
        account_count,
        monthly_income,
        monthly_expenses,
        savings_rate: health.savings_rate,
        health_score: health.score,
        upcoming_bills_count,
    }))
}

/// GET /api/dashboard/net-worth?months=12
pub async fn get_net_worth(
    user: AuthUser,
    State(state): State<AppState>,
    Query(params): Query<MonthsQuery>,
) -> Result<impl IntoResponse, AppError> {
    let portfolio_id = first_portfolio_id(&user, &state).await?;

    let accounts = state.account_repo().list_by_portfolio(portfolio_id).await?;
    let txn_rows = state
        .transaction_repo()
        .list_for_analysis(portfolio_id, None, None)
        .await?;
    let txns = to_analysis(&txn_rows);

    let snapshots: Vec<AccountSnapshot> = accounts
        .iter()
        .map(|a| AccountSnapshot {
            id: a.id,
            opening_balance: a.opening_balance,
            account_type: a.account_type,
            is_archived: a.is_archived,
        })
        .collect();

    let today = Utc::now().date_naive();
    let start = today
        .checked_sub_months(Months::new(params.months as u32))
        .unwrap_or(today);

    let series = compute_net_worth_series(&snapshots, &txns, start, today);
    let entries: Vec<NetWorthEntry> = series
        .into_iter()
        .map(|p| NetWorthEntry {
            date: p.date,
            total: p.total,
            assets: p.assets,
            liabilities: p.liabilities,
        })
        .collect();

    Ok(Json(entries))
}

/// GET /api/dashboard/cashflow?months=12
pub async fn get_cashflow(
    user: AuthUser,
    State(state): State<AppState>,
    Query(params): Query<MonthsQuery>,
) -> Result<impl IntoResponse, AppError> {
    let portfolio_id = first_portfolio_id(&user, &state).await?;

    let txn_rows = state
        .transaction_repo()
        .list_for_analysis(portfolio_id, None, None)
        .await?;
    let txns = to_analysis(&txn_rows);

    let cashflow = compute_monthly_cashflow(&txns, params.months);
    let entries: Vec<CashFlowEntry> = cashflow
        .into_iter()
        .map(|c| CashFlowEntry {
            month: c.month,
            income: c.income,
            expenses: c.expenses,
            net: c.net,
        })
        .collect();

    Ok(Json(entries))
}

/// GET /api/dashboard/health-score
pub async fn get_health_score(
    user: AuthUser,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let portfolio_id = first_portfolio_id(&user, &state).await?;

    let accounts = state.account_repo().list_by_portfolio(portfolio_id).await?;
    let txn_rows = state
        .transaction_repo()
        .list_for_analysis(portfolio_id, None, None)
        .await?;
    let txns = to_analysis(&txn_rows);

    // Compute assets/liabilities from today's actual balances via
    // the shared canonical-amount split (ADR-018).
    let mut total_assets = Decimal::ZERO;
    let mut total_liabilities = Decimal::ZERO;
    let mut liquid_savings = Decimal::ZERO;
    for acct in &accounts {
        if acct.is_archived {
            continue;
        }
        let balance = state.account_repo().compute_balance(acct.id).await?;
        let (a, l) = AccountRole::classify_balance(acct.account_type, balance);
        total_assets += a;
        total_liabilities += l;
        if matches!(
            acct.account_type,
            AccountType::Checking | AccountType::Savings | AccountType::Cash
        ) {
            liquid_savings += balance;
        }
    }

    let cashflow = compute_monthly_cashflow(&txns, 1);
    let monthly_income = cashflow.last().map(|c| c.income).unwrap_or_default();
    let monthly_expenses = cashflow.last().map(|c| c.expenses).unwrap_or_default();

    let cashflow_3 = compute_monthly_cashflow(&txns, 3);
    let avg_expenses = if cashflow_3.is_empty() {
        Decimal::ZERO
    } else {
        let total: Decimal = cashflow_3.iter().map(|c| c.expenses).sum();
        total / Decimal::from(cashflow_3.len() as i64)
    };

    let last_month_expenses = cashflow.last().map(|c| c.expenses).unwrap_or_default();

    let health = compute_health_score(&HealthScoreInput {
        monthly_income,
        monthly_expenses,
        total_assets,
        total_liabilities,
        liquid_savings,
        avg_monthly_expenses: avg_expenses,
        last_month_expenses,
    });

    Ok(Json(health))
}

/// GET /api/dashboard/spending?month=2026-04
/// When `month` is omitted, returns all-time spending across all transactions.
pub async fn get_spending(
    user: AuthUser,
    State(state): State<AppState>,
    Query(params): Query<MonthQuery>,
) -> Result<impl IntoResponse, AppError> {
    let portfolio_id = first_portfolio_id(&user, &state).await?;

    let txn_rows = if let Some(ref month_str) = params.month {
        let month = parse_month(&Some(month_str.clone()))?;
        let end = next_month_start(month);
        state
            .transaction_repo()
            .list_for_analysis(portfolio_id, Some(month), Some(end))
            .await?
    } else {
        state
            .transaction_repo()
            .list_for_analysis(portfolio_id, None, None)
            .await?
    };

    // Aggregate expenses by category.
    let mut by_category: std::collections::HashMap<String, Decimal> =
        std::collections::HashMap::new();
    let mut total_expenses = Decimal::ZERO;

    for row in &txn_rows {
        if row.amount < Decimal::ZERO {
            let cat = row
                .category
                .clone()
                .unwrap_or_else(|| "uncategorized".to_string());
            let abs = row.amount.abs();
            *by_category.entry(cat).or_default() += abs;
            total_expenses += abs;
        }
    }

    let total_f = total_expenses.to_f64().unwrap_or(1.0);
    let mut entries: Vec<SpendingEntry> = by_category
        .into_iter()
        .map(|(category, amount)| {
            let pct = if total_f > 0.0 {
                amount.to_f64().unwrap_or(0.0) / total_f * 100.0
            } else {
                0.0
            };
            SpendingEntry {
                category,
                amount,
                percentage: pct,
            }
        })
        .collect();

    entries.sort_by_key(|e| std::cmp::Reverse(e.amount));

    Ok(Json(entries))
}

/// GET /api/dashboard/spending/subcategories?category=housing&month=2026-04
/// Returns spending breakdown by subcategory within a given parent category.
pub async fn get_subcategory_spending(
    user: AuthUser,
    State(state): State<AppState>,
    Query(params): Query<SubcategorySpendQuery>,
) -> Result<impl IntoResponse, AppError> {
    let portfolio_id = first_portfolio_id(&user, &state).await?;

    let txn_rows = if let Some(ref month_str) = params.month {
        let month = parse_month(&Some(month_str.clone()))?;
        let end = next_month_start(month);
        state
            .transaction_repo()
            .list_for_analysis(portfolio_id, Some(month), Some(end))
            .await?
    } else {
        state
            .transaction_repo()
            .list_for_analysis(portfolio_id, None, None)
            .await?
    };

    // Aggregate expenses by subcategory within the requested parent
    // category. Matches the Sankey's spending-link semantics (ADR-018):
    // direction = outflow, excluding rows already represented as
    // inter-account transfers and categories on the transfer-exclusion
    // list.
    use finima_core::TransactionDirection;
    let db_flows = state
        .flow_repo()
        .list_by_portfolio_month(
            portfolio_id,
            params
                .month
                .as_deref()
                .map(|m| parse_month(&Some(m.to_string())))
                .transpose()?
                .unwrap_or_else(|| NaiveDate::from_ymd_opt(1970, 1, 1).unwrap()),
        )
        .await
        .unwrap_or_default();
    let transfer_txn_ids: std::collections::HashSet<uuid::Uuid> = db_flows
        .iter()
        .flat_map(|f| {
            f.source_transaction_id
                .into_iter()
                .chain(f.target_transaction_id)
        })
        .collect();
    let transfer_categories: std::collections::HashSet<&str> = state
        .config()
        .sankey
        .transfer_categories
        .iter()
        .map(String::as_str)
        .collect();

    let mut by_subcategory: std::collections::HashMap<String, Decimal> =
        std::collections::HashMap::new();
    let mut total_in_category = Decimal::ZERO;

    for row in &txn_rows {
        if transfer_txn_ids.contains(&row.id) {
            continue;
        }
        if row.direction != Some(TransactionDirection::Outflow) {
            continue;
        }
        let cat = row.category.as_deref().unwrap_or("");
        if cat != params.category {
            continue;
        }
        if transfer_categories.contains(cat) {
            continue;
        }
        let sub = row
            .subcategory
            .clone()
            .unwrap_or_else(|| "other".to_string());
        let abs = row.amount.abs();
        *by_subcategory.entry(sub).or_default() += abs;
        total_in_category += abs;
    }

    let total_f = total_in_category.to_f64().unwrap_or(1.0);
    let mut entries: Vec<SubcategorySpendEntry> = by_subcategory
        .into_iter()
        .map(|(subcategory, amount)| {
            let pct = if total_f > 0.0 {
                amount.to_f64().unwrap_or(0.0) / total_f * 100.0
            } else {
                0.0
            };
            SubcategorySpendEntry {
                subcategory,
                amount,
                percentage: pct,
            }
        })
        .collect();

    entries.sort_by_key(|e| std::cmp::Reverse(e.amount));

    Ok(Json(entries))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn decimal_serializes_as_number() {
        let summary = DashboardSummary {
            net_worth: Decimal::from_str("12345.67").unwrap(),
            total_assets: Decimal::from_str("50000").unwrap(),
            total_liabilities: Decimal::from_str("37654.33").unwrap(),
            account_count: 3,
            monthly_income: Decimal::from_str("8000").unwrap(),
            monthly_expenses: Decimal::from_str("5000").unwrap(),
            savings_rate: 37.5,
            health_score: 72,
            upcoming_bills_count: 5,
        };
        let json = serde_json::to_string(&summary).unwrap();
        println!("DashboardSummary JSON: {}", json);
        // Verify net_worth is a number, not a string
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(
            v["net_worth"].is_number(),
            "net_worth should be a JSON number, got: {}",
            v["net_worth"]
        );
        assert!(
            v["total_assets"].is_number(),
            "total_assets should be a JSON number, got: {}",
            v["total_assets"]
        );

        let spending = SpendingEntry {
            category: "food".into(),
            amount: Decimal::from_str("234.56").unwrap(),
            percentage: 15.3,
        };
        let json2 = serde_json::to_string(&spending).unwrap();
        println!("SpendingEntry JSON: {}", json2);
        let v2: serde_json::Value = serde_json::from_str(&json2).unwrap();
        assert!(
            v2["amount"].is_number(),
            "amount should be a JSON number, got: {}",
            v2["amount"]
        );
    }
}
