use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx;
use uuid::Uuid;

use finima_auth::middleware::AuthUser;
use finima_core::models::Account;
use finima_core::services::sign_normalizer::{
    AccountContext, SignConvention, SignNormalizer,
};
use finima_core::traits::{AccountRepo, PortfolioRepo};
use finima_core::types::{AccountType, TransactionDirection};
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
    /// Move the account to a different portfolio.
    pub portfolio_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
pub struct AccountDetailResponse {
    #[serde(flatten)]
    pub account: Account,
    pub computed_balance: Decimal,
    pub transaction_count: i64,
    pub last_import_at: Option<DateTime<Utc>>,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// GET /api/accounts?portfolio_id=
///
/// List all non-archived accounts for a portfolio. Verifies the authenticated
/// user owns the portfolio. Returns enriched data including computed balances,
/// transaction counts, and last import dates.
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

    let mut results = Vec::with_capacity(accounts.len());
    for account in accounts {
        let id = account.id;

        let computed_balance = state.account_repo().compute_balance(id).await?;

        let transaction_count = sqlx::query_scalar::<_, Option<i64>>(
            "SELECT COUNT(*) FROM transactions WHERE account_id = $1",
        )
        .bind(id)
        .fetch_one(state.pool())
        .await
        .unwrap_or(None)
        .unwrap_or(0);

        let last_import_at = sqlx::query_scalar::<_, Option<DateTime<Utc>>>(
            r#"
            SELECT MAX(u.uploaded_at)
            FROM uploads u
            WHERE u.account_id = $1 AND u.status IN ('complete', 'categorizing')
            "#,
        )
        .bind(id)
        .fetch_one(state.pool())
        .await
        .unwrap_or(None);

        results.push(AccountDetailResponse {
            account,
            computed_balance,
            transaction_count,
            last_import_at,
        });
    }

    Ok(Json(results))
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
        sign_convention_override: None,
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

    let transaction_count = sqlx::query_scalar::<_, Option<i64>>(
        "SELECT COUNT(*) FROM transactions WHERE account_id = $1",
    )
    .bind(id)
    .fetch_one(state.pool())
    .await
    .unwrap_or(None)
    .unwrap_or(0);

    let last_import_at = sqlx::query_scalar::<_, Option<DateTime<Utc>>>(
        r#"
        SELECT MAX(u.uploaded_at)
        FROM uploads u
        WHERE u.account_id = $1 AND u.status IN ('complete', 'categorizing')
        "#,
    )
    .bind(id)
    .fetch_one(state.pool())
    .await
    .unwrap_or(None);

    Ok(Json(AccountDetailResponse {
        account,
        computed_balance,
        transaction_count,
        last_import_at,
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

    // If moving to a different portfolio, verify the user owns the target too.
    let target_portfolio_id = body.portfolio_id.unwrap_or(existing.portfolio_id);
    if target_portfolio_id != existing.portfolio_id {
        state
            .portfolio_repo()
            .verify_ownership(target_portfolio_id, user.user_id)
            .await?;
    }

    let updated_account = Account {
        id: existing.id,
        portfolio_id: target_portfolio_id,
        name: body.name.unwrap_or(existing.name),
        institution: body.institution.or(existing.institution),
        account_type: body.account_type.unwrap_or(existing.account_type),
        currency: body.currency.unwrap_or(existing.currency),
        opening_balance: body.opening_balance.unwrap_or(existing.opening_balance),
        is_primary_income: existing.is_primary_income,
        is_archived: existing.is_archived,
        notes: body.notes.or(existing.notes),
        created_at: existing.created_at,
        sign_convention_override: existing.sign_convention_override,
    };

    let result = state.account_repo().update(&updated_account).await?;
    Ok(Json(result))
}

/// POST /api/accounts/:id/set-primary
///
/// Designate an account as the primary income account. Clears the flag on all
/// other accounts in the same portfolio, then sets it on the target account.
pub async fn set_primary_income(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let account = state.account_repo().find_by_id(id).await?;

    state
        .portfolio_repo()
        .verify_ownership(account.portfolio_id, user.user_id)
        .await?;

    // Clear primary flag on all accounts in this portfolio.
    let siblings = state
        .account_repo()
        .list_by_portfolio(account.portfolio_id)
        .await?;
    for sibling in &siblings {
        if sibling.is_primary_income {
            state
                .account_repo()
                .set_primary_income(sibling.id, false)
                .await?;
        }
    }

    // Set primary flag on the target account.
    state.account_repo().set_primary_income(id, true).await?;

    let updated = state.account_repo().find_by_id(id).await?;
    Ok(Json(updated))
}

// ---------------------------------------------------------------------------
// Sign-convention override (per-account "Flip this account" UI action)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct SetSignOverrideRequest {
    /// Set to a specific convention to pin this account, or `null`
    /// to clear the override and fall back to the institution rule
    /// / autodetection / account-type default.
    pub convention: Option<SignConvention>,
}

#[derive(Debug, Serialize)]
pub struct SignOverrideResponse {
    pub account: Account,
    /// Number of transactions whose `direction` value flipped as a
    /// result of the change. Useful for the post-action toast.
    pub rows_renormalized: u64,
    pub flipped: u64,
}

/// PUT /api/accounts/:id/sign-override
///
/// Set or clear the per-account sign-convention override. When
/// changed, every existing transaction on the account is
/// re-normalized server-side so historical data reflects the new
/// convention without requiring re-import.
///
/// See ADR-018 for the resolution chain.
pub async fn set_sign_override(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<SetSignOverrideRequest>,
) -> Result<impl IntoResponse, AppError> {
    let account = state.account_repo().find_by_id(id).await?;
    state
        .portfolio_repo()
        .verify_ownership(account.portfolio_id, user.user_id)
        .await?;

    // Persist the override.
    state
        .account_repo()
        .set_sign_convention_override(id, req.convention)
        .await?;

    // Re-normalize this account's existing transactions. Build a
    // normalizer that includes the (possibly cleared) override so the
    // resolution chain reflects the post-update state.
    let mut rules = state.config().sign_conventions.clone().into_service_rules();
    if let Some(c) = req.convention {
        rules.by_account_id.insert(id, c);
    } else {
        rules.by_account_id.remove(&id);
    }
    let normalizer = SignNormalizer::new(rules);
    let ctx = AccountContext {
        account_id: id,
        account_type: account.account_type,
        institution: account.institution.clone(),
    };

    let txn_rows: Vec<(Uuid, Decimal, Option<TransactionDirection>)> = sqlx::query_as(
        "SELECT id, amount, direction FROM transactions WHERE account_id = $1",
    )
    .bind(id)
    .fetch_all(state.pool())
    .await?;

    let mut flipped: u64 = 0;
    let total = txn_rows.len() as u64;
    for (txn_id, amount, prev_direction) in txn_rows {
        let new_direction = normalizer.direction_for(&ctx, amount);
        if Some(new_direction) != prev_direction {
            flipped += 1;
        }
        sqlx::query("UPDATE transactions SET direction = $1 WHERE id = $2")
            .bind(new_direction.to_string())
            .bind(txn_id)
            .execute(state.pool())
            .await?;
    }

    let updated = state.account_repo().find_by_id(id).await?;
    Ok(Json(SignOverrideResponse {
        account: updated,
        rows_renormalized: total,
        flipped,
    }))
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
