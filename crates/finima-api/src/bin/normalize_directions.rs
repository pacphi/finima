//! Maintainer-only CLI to backfill `transactions.direction` for rows
//! that landed before the SignNormalizer pipeline existed, or after a
//! YAML rule change that affects an institution already in the DB.
//!
//! Usage:
//!
//! ```text
//! cargo run -p finima-api --bin finima-normalize-directions -- [--institution NAME] [--account-id UUID] [--dry-run]
//! ```
//!
//! Behavior:
//! - Selects every transaction with `direction IS NULL` (default), or
//!   restricts to a specific institution / account when filters are
//!   supplied.
//! - For each row, computes the direction via the same SignNormalizer
//!   the import pipeline uses (built from `sankey.yaml` +
//!   `accounts.sign_convention_override` once Phase 5.7 lands).
//! - Writes the direction back unless `--dry-run` is passed.
//!
//! End users never run this. It's a maintainer tool for one-time
//! backfills after schema or YAML changes.
//!
//! See ADR-018.

#[path = "../config.rs"]
#[allow(dead_code)]
mod config;

use std::collections::HashMap;

use finima_core::services::sign_normalizer::{
    AccountContext, SignConvention, SignNormalizer,
};
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
                args.account_id = Some(
                    Uuid::parse_str(&raw).expect("--account-id value must be a valid UUID"),
                );
            }
            "--dry-run" => args.dry_run = true,
            "--help" | "-h" => {
                println!(
                    "Usage: finima-normalize-directions \
                     [--institution NAME] [--account-id UUID] [--dry-run]"
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

    // Build base rules from YAML; per-account overrides are folded in
    // below, after we read each row's account.sign_convention_override.
    let mut rules = config.sign_conventions.clone().into_service_rules();

    // Pull all candidate rows. Joining accounts gives us the
    // account_type, institution, and any per-account override.
    let rows = sqlx::query(
        r#"
        SELECT
            t.id                            AS id,
            t.account_id                    AS account_id,
            t.amount                        AS amount,
            a.account_type                  AS account_type,
            a.institution                   AS institution,
            a.sign_convention_override      AS sign_convention_override
        FROM transactions t
        JOIN accounts a ON a.id = t.account_id
        WHERE t.direction IS NULL
          AND ($1::text IS NULL OR LOWER(a.institution) = LOWER($1))
          AND ($2::uuid IS NULL OR t.account_id = $2)
        "#,
    )
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

    for row in &rows {
        let id: Uuid = row.try_get("id")?;
        let account_id: Uuid = row.try_get("account_id")?;
        let amount: Decimal = row.try_get("amount")?;
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

        if !args.dry_run {
            sqlx::query("UPDATE transactions SET direction = $1 WHERE id = $2")
                .bind(direction.to_string())
                .bind(id)
                .execute(&pool)
                .await?;
        }
        updates_attempted += 1;
    }

    let prefix = if args.dry_run { "would update" } else { "updated" };
    println!(
        "{prefix} {} rows ({} inflow, {} outflow)",
        updates_attempted,
        by_direction
            .get(&TransactionDirection::Inflow)
            .copied()
            .unwrap_or(0),
        by_direction
            .get(&TransactionDirection::Outflow)
            .copied()
            .unwrap_or(0),
    );

    Ok(())
}
