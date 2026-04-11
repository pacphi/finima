use axum::extract::{Query, State};
use axum::response::IntoResponse;
use axum::Json;
use chrono::{Datelike, Months, NaiveDate, Utc};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use finima_analysis::{
    compute_health_score, compute_monthly_cashflow, compute_net_worth_series, AccountSnapshot,
    HealthScoreInput,
};
use finima_auth::middleware::AuthUser;
use finima_core::traits::AccountRepo;
use finima_core::AppError;

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

    // Compute net worth from current balances.
    let today = Utc::now().date_naive();
    let snapshots: Vec<AccountSnapshot> = accounts
        .iter()
        .map(|a| AccountSnapshot {
            id: a.id,
            opening_balance: a.opening_balance,
            account_type: a.account_type,
            is_archived: a.is_archived,
        })
        .collect();

    let nw_series = compute_net_worth_series(&snapshots, &txns, today, today);
    let net_worth = nw_series.first().map(|p| p.total).unwrap_or_default();
    let total_assets = nw_series.first().map(|p| p.assets).unwrap_or_default();
    let total_liabilities = nw_series.first().map(|p| p.liabilities).unwrap_or_default();

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

    // Liquid savings: sum of checking + savings balances.
    let mut liquid_savings = Decimal::ZERO;
    for acct in &accounts {
        if !acct.is_archived
            && matches!(
                acct.account_type,
                finima_core::types::AccountType::Checking
                    | finima_core::types::AccountType::Savings
                    | finima_core::types::AccountType::Cash
            )
        {
            let balance = state.account_repo().compute_balance(acct.id).await?;
            liquid_savings += balance;
        }
    }

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

    Ok(Json(DashboardSummary {
        net_worth,
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

/// GET /api/dashboard/spending?month=2026-04
pub async fn get_spending(
    user: AuthUser,
    State(state): State<AppState>,
    Query(params): Query<MonthQuery>,
) -> Result<impl IntoResponse, AppError> {
    let portfolio_id = first_portfolio_id(&user, &state).await?;

    let month = parse_month(&params.month)?;

    // Get the last day of the month.
    let end = if month.month() == 12 {
        NaiveDate::from_ymd_opt(month.year() + 1, 1, 1).unwrap()
    } else {
        NaiveDate::from_ymd_opt(month.year(), month.month() + 1, 1).unwrap()
    };

    let txn_rows = state
        .transaction_repo()
        .list_for_analysis(portfolio_id, Some(month), Some(end))
        .await?;

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

    entries.sort_by(|a, b| b.amount.cmp(&a.amount));

    Ok(Json(entries))
}
