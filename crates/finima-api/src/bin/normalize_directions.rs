//! Maintainer-only CLI to backfill `transactions.direction` for rows
//! that landed before the SignNormalizer pipeline existed, AND to
//! canonicalize `transactions.amount` for rows that landed before
//! the import pipeline started storing amounts in the canonical
//! `positive_means_inflow` convention.
//!
//! Usage:
//!
//! ```text
//! cargo run -p finima-api --bin finima-normalize-directions -- \
//!     [--institution NAME] [--account-id UUID] \
//!     [--canonicalize-amounts] [--dry-run]
//! ```
//!
//! Modes:
//!
//! - Default (direction-only backfill):
//!   Selects rows with `direction IS NULL`, computes the direction
//!   via the same SignNormalizer the import pipeline uses, and
//!   writes it back. Idempotent — already-populated rows are
//!   skipped.
//!
//! - `--force`:
//!   Extends the default mode to **every** selected row, not just
//!   those with `direction IS NULL`. Use after a YAML rule change
//!   that invalidates previously computed directions on an existing
//!   institution (e.g. adding the `bank of america` rule after BofA
//!   rows have already been imported under the wrong default). Rows
//!   whose recomputed direction matches the stored one pay a single
//!   UPDATE but no logical change.
//!
//! - `--canonicalize-amounts`:
//!   Iterates every account whose effective sign convention resolves
//!   to `PositiveMeansOutflow` (Amex/Discover-style) and negates
//!   `amount` on every row so the stored sign matches the canonical
//!   convention: positive == inflow, negative == outflow. Idempotent
//!   via the `(account_id, direction, amount)` sign invariant: rows
//!   whose sign is already consistent with `direction` are left
//!   alone. Filters (`--institution`, `--account-id`) apply.
//!
//! End users never run this. It's a maintainer tool for one-time
//! backfills after schema, YAML, or canonical-convention changes.
//!
//! See ADR-018.

#[path = "../config.rs"]
#[allow(dead_code)]
mod config;

use std::collections::HashMap;

use finima_core::services::sign_normalizer::{AccountContext, SignConvention, SignNormalizer};
use finima_core::types::{AccountType, TransactionDirection};
use rust_decimal::Decimal;
use sqlx::postgres::PgPoolOptions;
use sqlx::Row;
use uuid::Uuid;

use config::load_config;

#[derive(Debug, Default)]
struct Args {
    institution: Option<String>,
    account_id: Option<Uuid>,
    canonicalize_amounts: bool,
    force: bool,
    dry_run: bool,
}

fn parse_args() -> Args {
    let mut args = Args::default();
    let mut iter = std::env::args().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--institution" => args.institution = iter.next(),
            "--account-id" => {
                let raw = iter.next().expect("--account-id requires a UUID value");
                args.account_id =
                    Some(Uuid::parse_str(&raw).expect("--account-id value must be a valid UUID"));
            }
            "--canonicalize-amounts" => args.canonicalize_amounts = true,
            "--force" => args.force = true,
            "--dry-run" => args.dry_run = true,
            "--help" | "-h" => {
                println!(
                    "Usage: finima-normalize-directions \
                     [--institution NAME] [--account-id UUID] \
                     [--canonicalize-amounts] [--force] [--dry-run]"
                );
                std::process::exit(0);
            }
            other => panic!("unknown argument: {}", other),
        }
    }
    args
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = parse_args();
    let config = load_config()?;

    let pool = PgPoolOptions::new()
        .max_connections(config.database.max_connections)
        .connect(&config.database.resolved_url())
        .await?;

    if args.canonicalize_amounts {
        canonicalize_amounts_pass(&pool, &config, &args).await?;
        return Ok(());
    }

    // Build base rules from YAML; per-account overrides are folded in
    // below, after we read each row's account.sign_convention_override.
    let mut rules = config.sign_conventions.clone().into_service_rules();

    // Pull all candidate rows. In --force mode we scan every row
    // (and let UPDATEs no-op when the recomputed direction already
    // matches); otherwise we limit to rows with NULL direction.
    // Joining accounts gives us the account_type, institution, and
    // any per-account override in one query.
    let direction_filter = if args.force {
        "(t.direction IS NULL OR TRUE)"
    } else {
        "t.direction IS NULL"
    };
    let query_sql = format!(
        r#"
        SELECT
            t.id                            AS id,
            t.account_id                    AS account_id,
            t.amount                        AS amount,
            t.direction                     AS direction,
            a.account_type                  AS account_type,
            a.institution                   AS institution,
            a.sign_convention_override      AS sign_convention_override
        FROM transactions t
        JOIN accounts a ON a.id = t.account_id
        WHERE {direction_filter}
          AND ($1::text IS NULL OR LOWER(a.institution) = LOWER($1))
          AND ($2::uuid IS NULL OR t.account_id = $2)
        "#
    );
    let rows = sqlx::query(&query_sql)
        .bind(args.institution.as_deref())
        .bind(args.account_id)
        .fetch_all(&pool)
        .await?;

    // Fold per-account overrides into the rules so they win over
    // institution defaults. Done once across the result set.
    for row in &rows {
        let acct: Uuid = row.try_get("account_id")?;
        if rules.by_account_id.contains_key(&acct) {
            continue;
        }
        let override_val: Option<SignConvention> = row.try_get("sign_convention_override")?;
        if let Some(c) = override_val {
            rules.by_account_id.insert(acct, c);
        }
    }
    let normalizer = SignNormalizer::new(rules);

    println!("candidates: {}", rows.len());
    if rows.is_empty() {
        println!("nothing to do.");
        return Ok(());
    }

    let mut by_direction: HashMap<TransactionDirection, u64> = HashMap::new();
    let mut updates_attempted: u64 = 0;
    let mut already_correct: u64 = 0;

    for row in &rows {
        let id: Uuid = row.try_get("id")?;
        let account_id: Uuid = row.try_get("account_id")?;
        let amount: Decimal = row.try_get("amount")?;
        let prev_direction: Option<TransactionDirection> = row.try_get("direction")?;
        let account_type: AccountType = row.try_get("account_type")?;
        let institution: Option<String> = row.try_get("institution")?;
        let _override: Option<SignConvention> = row.try_get("sign_convention_override")?;

        let ctx = AccountContext {
            account_id,
            account_type,
            institution: institution.clone(),
        };
        let direction = normalizer.direction_for(&ctx, amount);
        *by_direction.entry(direction).or_default() += 1;

        // In --force mode many rows will already have the correct
        // direction; skip the UPDATE entirely for those so the pass
        // stays cheap on large tables.
        if prev_direction == Some(direction) {
            already_correct += 1;
            continue;
        }

        if !args.dry_run {
            sqlx::query("UPDATE transactions SET direction = $1 WHERE id = $2")
                .bind(direction.to_string())
                .bind(id)
                .execute(&pool)
                .await?;
        }
        updates_attempted += 1;
    }

    let prefix = if args.dry_run {
        "would update"
    } else {
        "updated"
    };
    println!(
        "{prefix} {} rows ({} inflow, {} outflow); {} already correct, skipped",
        updates_attempted,
        by_direction
            .get(&TransactionDirection::Inflow)
            .copied()
            .unwrap_or(0),
        by_direction
            .get(&TransactionDirection::Outflow)
            .copied()
            .unwrap_or(0),
        already_correct,
    );

    Ok(())
}

/// One-time canonicalization pass: for every account whose effective
/// sign convention resolves to `PositiveMeansOutflow`, flip the sign
/// of `transactions.amount` so the stored value matches the canonical
/// `PositiveMeansInflow` convention (positive == inflow).
///
/// Idempotency: we detect whether a flip is needed per account by
/// checking whether any `direction='inflow'` row currently has a
/// negative amount OR any `direction='outflow'` row has a positive
/// amount. If the invariant already holds, the account is skipped.
///
/// This pass assumes `direction` has already been populated (via the
/// default mode of this CLI, or the import pipeline). Rows with
/// `direction IS NULL` are left alone.
async fn canonicalize_amounts_pass(
    pool: &sqlx::PgPool,
    config: &config::AppConfig,
    args: &Args,
) -> Result<(), Box<dyn std::error::Error>> {
    let base_rules = config.sign_conventions.clone().into_service_rules();

    // Pull all accounts that match the filter.
    let account_rows = sqlx::query(
        r#"
        SELECT id, account_type, institution, sign_convention_override
        FROM accounts
        WHERE is_archived = false
          AND ($1::text IS NULL OR LOWER(institution) = LOWER($1))
          AND ($2::uuid IS NULL OR id = $2)
        "#,
    )
    .bind(args.institution.as_deref())
    .bind(args.account_id)
    .fetch_all(pool)
    .await?;

    // Pre-build a normalizer that contains every account's override so
    // we can resolve the effective convention without rebuilding per
    // account.
    let mut rules = base_rules.clone();
    for row in &account_rows {
        let acct: Uuid = row.try_get("id")?;
        let override_val: Option<SignConvention> = row.try_get("sign_convention_override")?;
        if let Some(c) = override_val {
            rules.by_account_id.insert(acct, c);
        }
    }
    let normalizer = SignNormalizer::new(rules);

    let mut accounts_flipped: u64 = 0;
    let mut accounts_already_canonical: u64 = 0;
    let mut rows_flipped: u64 = 0;

    for row in &account_rows {
        let account_id: Uuid = row.try_get("id")?;
        let account_type: AccountType = row.try_get("account_type")?;
        let institution: Option<String> = row.try_get("institution")?;

        let ctx = AccountContext {
            account_id,
            account_type,
            institution: institution.clone(),
        };

        // We only flip accounts whose effective convention is the
        // non-canonical PositiveMeansOutflow.
        if normalizer.resolve_convention(&ctx) == SignConvention::PositiveMeansInflow {
            accounts_already_canonical += 1;
            continue;
        }

        // Count how many rows are currently non-canonical on this
        // account. A canonical row satisfies:
        //   direction='inflow'  -> amount >= 0
        //   direction='outflow' -> amount <= 0
        let non_canonical_count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM transactions
            WHERE account_id = $1
              AND direction IS NOT NULL
              AND (
                  (direction = 'inflow'  AND amount < 0)
                  OR (direction = 'outflow' AND amount > 0)
              )
            "#,
        )
        .bind(account_id)
        .fetch_one(pool)
        .await?;

        if non_canonical_count == 0 {
            // Already canonicalized in a previous run — skip.
            accounts_already_canonical += 1;
            continue;
        }

        if args.dry_run {
            println!(
                "would flip {} rows on account {} (effective: positive_means_outflow)",
                non_canonical_count, account_id,
            );
        } else {
            let updated = sqlx::query(
                r#"
                UPDATE transactions
                SET amount = -amount
                WHERE account_id = $1
                  AND direction IS NOT NULL
                  AND (
                      (direction = 'inflow'  AND amount < 0)
                      OR (direction = 'outflow' AND amount > 0)
                  )
                "#,
            )
            .bind(account_id)
            .execute(pool)
            .await?
            .rows_affected();
            rows_flipped += updated;
            println!("flipped {} rows on account {}", updated, account_id);
        }

        accounts_flipped += 1;
    }

    let verb = if args.dry_run {
        "would flip"
    } else {
        "flipped"
    };
    println!(
        "\n{verb} {} rows across {} accounts; {} accounts already canonical.",
        rows_flipped, accounts_flipped, accounts_already_canonical,
    );
    Ok(())
}
