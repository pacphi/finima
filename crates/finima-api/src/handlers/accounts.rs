use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use finima_auth::middleware::AuthUser;
use finima_core::models::Account;
use finima_core::traits::{AccountRepo, PortfolioRepo};
use finima_core::types::AccountType;
use finima_core::AppError;

use crate::state::AppState;

// ---------------------------------------------------------------------------
// Request / response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct ListAccountsQuery {
    pub portfolio_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct CreateAccountRequest {
    pub portfolio_id: Uuid,
    pub name: String,
    pub institution: Option<String>,
    pub account_type: AccountType,
    pub currency: Option<String>,
    pub opening_balance: Option<Decimal>,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateAccountRequest {
    pub name: Option<String>,
    pub institution: Option<String>,
    pub account_type: Option<AccountType>,
    pub currency: Option<String>,
    pub opening_balance: Option<Decimal>,
    pub notes: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AccountDetailResponse {
    #[serde(flatten)]
    pub account: Account,
    pub computed_balance: Decimal,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// GET /api/accounts?portfolio_id=
///
/// List all non-archived accounts for a portfolio. Verifies the authenticated
/// user owns the portfolio.
pub async fn list_accounts(
    user: AuthUser,
    State(state): State<AppState>,
    Query(params): Query<ListAccountsQuery>,
) -> Result<impl IntoResponse, AppError> {
    state
        .portfolio_repo()
        .verify_ownership(params.portfolio_id, user.user_id)
        .await?;

    let accounts = state
        .account_repo()
        .list_by_portfolio(params.portfolio_id)
        .await?;

    Ok(Json(accounts))
}

/// POST /api/accounts
///
/// Create a new account within a portfolio. Verifies portfolio ownership.
pub async fn create_account(
    user: AuthUser,
    State(state): State<AppState>,
    Json(body): Json<CreateAccountRequest>,
) -> Result<impl IntoResponse, AppError> {
    let name = body.name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::BadRequest("Account name is required".to_string()));
    }

    state
        .portfolio_repo()
        .verify_ownership(body.portfolio_id, user.user_id)
        .await?;

    let account = Account {
        id: Uuid::new_v4(),
        portfolio_id: body.portfolio_id,
        name,
        institution: body.institution,
        account_type: body.account_type,
        currency: body.currency.unwrap_or_else(|| "USD".to_string()),
        opening_balance: body.opening_balance.unwrap_or(Decimal::ZERO),
        is_primary_income: false,
        is_archived: false,
        notes: body.notes,
        created_at: chrono::Utc::now(),
    };

    let created = state.account_repo().create(&account).await?;
    Ok((StatusCode::CREATED, Json(created)))
}

/// GET /api/accounts/:id
///
/// Get account detail with its computed balance. Verifies ownership through
/// the account's portfolio.
pub async fn get_account(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let account = state.account_repo().find_by_id(id).await?;

    state
        .portfolio_repo()
        .verify_ownership(account.portfolio_id, user.user_id)
        .await?;

    let computed_balance = state.account_repo().compute_balance(id).await?;

    Ok(Json(AccountDetailResponse {
        account,
        computed_balance,
    }))
}

/// PUT /api/accounts/:id
///
/// Update an account's mutable fields. Verifies ownership through the
/// account's portfolio.
pub async fn update_account(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateAccountRequest>,
) -> Result<impl IntoResponse, AppError> {
    let existing = state.account_repo().find_by_id(id).await?;

    state
        .portfolio_repo()
        .verify_ownership(existing.portfolio_id, user.user_id)
        .await?;

    let updated_account = Account {
        id: existing.id,
        portfolio_id: existing.portfolio_id,
        name: body.name.unwrap_or(existing.name),
        institution: body.institution.or(existing.institution),
        account_type: body.account_type.unwrap_or(existing.account_type),
        currency: body.currency.unwrap_or(existing.currency),
        opening_balance: body.opening_balance.unwrap_or(existing.opening_balance),
        is_primary_income: existing.is_primary_income,
        is_archived: existing.is_archived,
        notes: body.notes.or(existing.notes),
        created_at: existing.created_at,
    };

    let result = state.account_repo().update(&updated_account).await?;
    Ok(Json(result))
}

/// DELETE /api/accounts/:id
///
/// Archive (soft-delete) an account. Verifies ownership through the account's
/// portfolio.
pub async fn delete_account(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let account = state.account_repo().find_by_id(id).await?;

    state
        .portfolio_repo()
        .verify_ownership(account.portfolio_id, user.user_id)
        .await?;

    state.account_repo().archive(id).await?;

    Ok(StatusCode::NO_CONTENT)
}
