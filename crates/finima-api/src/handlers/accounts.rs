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
use finima_core::services::sign_normalizer::{AccountContext, SignConvention, SignNormalizer};
use finima_core::traits::{AccountRepo, PortfolioRepo};
use finima_core::types::AccountType;
use finima_core::AppError;

use crate::state::AppState;

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Build an [`AccountContext`] from a full [`Account`] — the
/// SignNormalizer input shape. Centralized so callers don't forget a
/// field.
fn account_context(account: &Account) -> AccountContext {
    AccountContext {
        account_id: account.id,
        account_type: account.account_type,
        institution: account.institution.clone(),
    }
}

/// Build a [`SignNormalizer`] reflecting the base YAML rules merged
/// with the account's persisted override (if any). Useful when the
/// caller needs to answer "what convention applies to this account
/// right now?" without re-implementing the resolution chain.
fn normalizer_with_account_override(state: &AppState, account: &Account) -> SignNormalizer {
    let mut rules = state.config().sign_conventions.clone().into_service_rules();
    if let Some(c) = account.sign_convention_override {
        rules.by_account_id.insert(account.id, c);
    }
    SignNormalizer::new(rules)
}

/// Resolve the sign convention that would be applied for a fresh
/// import on the given account. Autodetection is skipped (no file
/// sample available here) so the result reflects "what rule fires
/// when no file has been offered yet".
fn effective_convention_for(state: &AppState, account: &Account) -> SignConvention {
    normalizer_with_account_override(state, account).resolve_convention(&account_context(account))
}

/// Assemble the [`AccountDetailResponse`] for a single account. Used
/// by every handler that returns one (list, get, update, set sign
/// override). Keeps the three follow-on SELECTs in one place so
/// they can't drift across handlers.
async fn build_account_detail(
    state: &AppState,
    account: Account,
) -> Result<AccountDetailResponse, AppError> {
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

    let effective_sign_convention = effective_convention_for(state, &account);

    Ok(AccountDetailResponse {
        account,
        computed_balance,
        transaction_count,
        last_import_at,
        effective_sign_convention,
    })
}

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
    /// The sign convention that would be used for a fresh import on
    /// this account *right now*, after running the full resolution
    /// chain (per-account override -> institution rule ->
    /// account-type default; autodetection is skipped since no file
    /// is present). The UI uses this to decide how to interpret a
    /// Flip click when no per-account override is set.
    pub effective_sign_convention: SignConvention,
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
        results.push(build_account_detail(&state, account).await?);
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

    Ok(Json(build_account_detail(&state, account).await?))
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
    /// The refreshed account detail (balance, counts, last-import),
    /// not just the raw account row. Returning the full detail lets
    /// the client swap this into state in one shot without a second
    /// round-trip to GET /api/accounts/:id.
    pub account: AccountDetailResponse,
    /// Number of transactions whose `direction` value flipped as a
    /// result of the change. Useful for the post-action toast.
    pub rows_renormalized: u64,
    pub flipped: u64,
}

/// PUT /api/accounts/:id/sign-override
///
/// Set or clear the per-account sign-convention override. When the
/// effective convention changes as a result, every existing
/// transaction on the account has its canonical `amount` sign
/// inverted and its `direction` flipped so historical data reflects
/// the new interpretation without requiring a re-import.
///
/// Because stored amounts are already canonical
/// (`positive_means_inflow`; see ADR-018), "re-interpreting" past
/// rows under a flipped convention is equivalent to negating the
/// amount on every row — there is no need to recover the original
/// raw sign. If the convention resolves to the same value before
/// and after the override change (e.g. the user pins what was
/// already the default) no row-level work is performed.
///
/// The override change and the per-row update run in a single
/// database transaction so a failure partway through cannot leave
/// rows in a mixed-convention state.
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

    // Resolve the pre-change and post-change effective conventions
    // for this account. If they match, persist the override but skip
    // the row-flip step entirely.
    let base_rules = state.config().sign_conventions.clone().into_service_rules();
    let ctx = account_context(&account);

    let mut old_rules = base_rules.clone();
    if let Some(c) = account.sign_convention_override {
        old_rules.by_account_id.insert(id, c);
    }
    let old_convention = SignNormalizer::new(old_rules).resolve_convention(&ctx);

    let mut new_rules = base_rules;
    if let Some(c) = req.convention {
        new_rules.by_account_id.insert(id, c);
    }
    let new_convention = SignNormalizer::new(new_rules).resolve_convention(&ctx);

    let effective_convention_changed = old_convention != new_convention;

    let total: u64 = sqlx::query_scalar::<_, Option<i64>>(
        "SELECT COUNT(*)::bigint FROM transactions WHERE account_id = $1",
    )
    .bind(id)
    .fetch_one(state.pool())
    .await
    .unwrap_or(None)
    .unwrap_or(0) as u64;

    // One transaction: override write + conditional bulk flip.
    let mut tx = state.pool().begin().await?;

    sqlx::query("UPDATE accounts SET sign_convention_override = $2 WHERE id = $1")
        .bind(id)
        .bind(req.convention)
        .execute(&mut *tx)
        .await?;

    let flipped: u64 = if effective_convention_changed {
        // Bulk update: negate amount and flip direction on every row.
        // The DB handles the direction swap via a CASE expression so
        // we avoid round-tripping rows to Rust just to invert them.
        let rows_affected = sqlx::query(
            r#"
            UPDATE transactions
            SET amount    = -amount,
                direction = CASE direction
                              WHEN 'inflow'  THEN 'outflow'
                              WHEN 'outflow' THEN 'inflow'
                              ELSE direction
                            END
            WHERE account_id = $1
            "#,
        )
        .bind(id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        rows_affected
    } else {
        0
    };

    tx.commit().await?;

    // Build the full AccountDetailResponse so the client can drop it
    // straight into its account state without needing a follow-up
    // GET /api/accounts/:id.
    let updated = state.account_repo().find_by_id(id).await?;
    let detail = build_account_detail(&state, updated).await?;

    Ok(Json(SignOverrideResponse {
        account: detail,
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

/// DELETE /api/accounts/:id/purge
///
/// **Destructive.** Permanently deletes the account and every associated row
/// (transactions, uploads, account_flows, flow_patterns via ON DELETE CASCADE)
/// as well as any object-storage artifacts for the account's uploads. Cannot
/// be undone. The caller is expected to have prompted the user for
/// confirmation client-side.
pub async fn purge_account(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let account = state.account_repo().find_by_id(id).await?;

    state
        .portfolio_repo()
        .verify_ownership(account.portfolio_id, user.user_id)
        .await?;

    // Best-effort: remove any S3 objects for this account's uploads before the
    // DB cascade wipes the rows. Column mapping JSON stores the `s3_key` set
    // at preview time (see uploads::create_upload). Failures here must not
    // block deletion — we log and continue.
    let s3_keys: Vec<String> = sqlx::query_scalar::<_, Option<String>>(
        r#"
        SELECT column_mapping->>'s3_key'
        FROM uploads
        WHERE account_id = $1
          AND column_mapping ? 's3_key'
        "#,
    )
    .bind(id)
    .fetch_all(state.pool())
    .await
    .unwrap_or_default()
    .into_iter()
    .flatten()
    .collect();

    for key in &s3_keys {
        if let Err(e) = state.object_storage().delete_object(key).await {
            tracing::warn!(
                account_id = %id,
                s3_key = %key,
                error = %e,
                "Failed to delete S3 object during account purge — continuing"
            );
        }
    }

    // Hard-delete the account. ON DELETE CASCADE handles transactions, uploads,
    // account_flows, flow_patterns. savings_goals.linked_account_id is set to
    // NULL (the goal itself is retained by design — see migration 010).
    state.account_repo().delete(id).await?;

    tracing::info!(
        account_id = %id,
        s3_objects_deleted = s3_keys.len(),
        "Purged account and all associated data"
    );

    Ok(StatusCode::NO_CONTENT)
}
