use chrono::{Datelike, NaiveDate, Utc};
use uuid::Uuid;

use finima_analysis::TransactionForAnalysis;
use finima_auth::middleware::AuthUser;
use finima_core::traits::PortfolioRepo;
use finima_core::AppError;
use finima_db::TransactionForAnalysisRow;

use crate::state::AppState;

/// Get the user's first portfolio ID or return NotFound.
pub async fn first_portfolio_id(user: &AuthUser, state: &AppState) -> Result<Uuid, AppError> {
    let portfolios = state.portfolio_repo().list_by_user(user.user_id).await?;
    portfolios.first().map(|p| p.id).ok_or(AppError::NotFound)
}

/// Resolve the portfolio a request should operate on.
///
/// If `requested` is provided, verify the caller owns it and return it.
/// Otherwise fall back to the user's first portfolio.
///
/// This is what per-request handlers should call so the frontend's
/// selected portfolio actually drives the response. `first_portfolio_id`
/// remains available for internal callers that have no request context.
pub async fn resolve_portfolio_id(
    user: &AuthUser,
    state: &AppState,
    requested: Option<Uuid>,
) -> Result<Uuid, AppError> {
    match requested {
        Some(id) => {
            state
                .portfolio_repo()
                .verify_ownership(id, user.user_id)
                .await?;
            Ok(id)
        }
        None => first_portfolio_id(user, state).await,
    }
}

/// Convert DB rows to analysis-compatible structs.
pub fn to_analysis(rows: &[TransactionForAnalysisRow]) -> Vec<TransactionForAnalysis> {
    rows.iter()
        .map(|r| TransactionForAnalysis {
            id: r.id,
            date: r.date,
            amount: r.amount,
            description: r.description.clone(),
            merchant_name: r.merchant_name.clone(),
            category: r.category.clone(),
            account_id: Some(r.account_id),
        })
        .collect()
}

/// Parse a month string (YYYY-MM) into the first day of that month.
/// Falls back to the current month if not provided.
pub fn parse_month(month_str: &Option<String>) -> Result<NaiveDate, AppError> {
    match month_str {
        Some(s) => {
            let parts: Vec<&str> = s.split('-').collect();
            if parts.len() < 2 {
                return Err(AppError::BadRequest(
                    "month must be in YYYY-MM format".to_string(),
                ));
            }
            let year: i32 = parts[0]
                .parse()
                .map_err(|_| AppError::BadRequest("Invalid year".to_string()))?;
            let month: u32 = parts[1]
                .parse()
                .map_err(|_| AppError::BadRequest("Invalid month".to_string()))?;
            NaiveDate::from_ymd_opt(year, month, 1)
                .ok_or_else(|| AppError::BadRequest("Invalid month date".to_string()))
        }
        None => {
            let today = Utc::now().date_naive();
            Ok(NaiveDate::from_ymd_opt(today.year(), today.month(), 1).unwrap())
        }
    }
}
