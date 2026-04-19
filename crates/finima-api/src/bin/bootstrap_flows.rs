//! Maintainer-only CLI that seeds the `flow_patterns` table from
//! previously-confirmed `account_flows` rows so SONA-backed flow
//! detection has a warm starting set per portfolio.
//!
//! ADR-017 Phase 2.C. Mirrors the shape of `bootstrap_tier2`.
//!
//! Usage:
//!
//! ```text
//! cargo run -p finima-api --bin bootstrap_flows -- \
//!     [--portfolio-id UUID] [--dry-run] [--limit N]
//! ```
//!
//! End users never run this.

#[path = "../config.rs"]
#[allow(dead_code)]
mod config;

use std::time::Instant;

use sqlx::postgres::PgPoolOptions;
use sqlx::Row;
use uuid::Uuid;

use finima_db::repos::{FlowPatternRepo, NewFlowPattern};

use config::load_config;

#[derive(Debug, Default)]
struct Args {
    portfolio_id: Option<Uuid>,
    dry_run: bool,
    limit_override: Option<usize>,
}

fn parse_args() -> Args {
    let mut args = Args::default();
    let mut iter = std::env::args().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--portfolio-id" => {
                let raw = iter.next().expect("--portfolio-id requires a UUID value");
                args.portfolio_id =
                    Some(Uuid::parse_str(&raw).expect("--portfolio-id value must be a valid UUID"));
            }
            "--dry-run" => args.dry_run = true,
            "--limit" => {
                let raw = iter.next().expect("--limit requires a number");
                args.limit_override = Some(
                    raw.parse::<usize>()
                        .expect("--limit must be a non-negative integer"),
                );
            }
            "--help" | "-h" => {
                println!(
                    "Usage: bootstrap_flows \
                     [--portfolio-id UUID] [--dry-run] [--limit N]"
                );
                std::process::exit(0);
            }
            other => panic!("unknown argument: {}", other),
        }
    }
    args
}

#[derive(Debug, Default)]
struct BootstrapReport {
    offered: usize,
    inserted: usize,
    skipped: usize,
    elapsed_ms: u64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let args = parse_args();
    let app_config = load_config()?;

    let pool = PgPoolOptions::new()
        .max_connections(app_config.database.max_connections)
        .connect(&app_config.database.resolved_url())
        .await?;

    let flow_pattern_repo = FlowPatternRepo::new(pool.clone());

    tracing::info!(
        dry_run = args.dry_run,
        limit = ?args.limit_override,
        "flow-pattern bootstrap starting"
    );

    // Resolve target portfolios.
    let portfolio_ids: Vec<Uuid> = if let Some(pid) = args.portfolio_id {
        vec![pid]
    } else {
        sqlx::query("SELECT id FROM portfolios ORDER BY id")
            .fetch_all(&pool)
            .await?
            .into_iter()
            .map(|row| row.get::<Uuid, _>("id"))
            .collect()
    };

    if portfolio_ids.is_empty() {
        tracing::warn!("No portfolios found; nothing to bootstrap");
        return Ok(());
    }

    let mut total = BootstrapReport::default();

    for portfolio_id in &portfolio_ids {
        let t0 = Instant::now();

        // Confirmed two-sided flows only. `target_account_id` is NOT NULL
        // in the schema, but we keep the guard to be explicit.
        let rows = sqlx::query(
            r#"
            SELECT f.id,
                   f.source_account_id,
                   f.target_account_id,
                   f.source_transaction_id
              FROM account_flows f
             WHERE f.portfolio_id = $1
               AND f.is_confirmed = true
               AND f.target_account_id IS NOT NULL
               AND f.source_transaction_id IS NOT NULL
             ORDER BY f.flow_date DESC
            "#,
        )
        .bind(portfolio_id)
        .fetch_all(&pool)
        .await?;

        if rows.is_empty() {
            tracing::info!(
                portfolio_id = %portfolio_id,
                "no confirmed flows; skipping"
            );
            continue;
        }

        let max_examples = args.limit_override.unwrap_or(usize::MAX);

        let mut report = BootstrapReport::default();

        for row in rows.iter().take(max_examples) {
            report.offered += 1;

            let source_txn_id: Uuid = row.get("source_transaction_id");
            let source_account_id: Uuid = row.get("source_account_id");
            let target_account_id: Uuid = row.get("target_account_id");

            // Look up the description directly to avoid pulling the full
            // repo handle wiring into this bin.
            let desc: Option<String> =
                sqlx::query_scalar("SELECT description FROM transactions WHERE id = $1")
                    .bind(source_txn_id)
                    .fetch_optional(&pool)
                    .await?;

            let Some(description_text) = desc else {
                tracing::debug!(
                    portfolio_id = %portfolio_id,
                    txn_id = %source_txn_id,
                    "source transaction missing; skipping"
                );
                report.skipped += 1;
                continue;
            };

            if args.dry_run {
                continue;
            }

            let payload = NewFlowPattern {
                portfolio_id: *portfolio_id,
                description_text,
                source_account_id,
                target_account_id,
                confidence: 1.0,
                embedding: None,
                embedding_dim: None,
            };

            match flow_pattern_repo.upsert_confirmed(payload).await {
                Ok(_) => report.inserted += 1,
                Err(e) => {
                    tracing::warn!(
                        portfolio_id = %portfolio_id,
                        error = %e,
                        "upsert_confirmed failed"
                    );
                    report.skipped += 1;
                }
            }
        }

        report.elapsed_ms = t0.elapsed().as_millis() as u64;

        tracing::info!(
            portfolio_id = %portfolio_id,
            offered = report.offered,
            inserted = report.inserted,
            skipped = report.skipped,
            elapsed_ms = report.elapsed_ms,
            "flow-pattern bootstrap complete"
        );

        total.offered += report.offered;
        total.inserted += report.inserted;
        total.skipped += report.skipped;
    }

    tracing::info!(
        portfolios = portfolio_ids.len(),
        offered = total.offered,
        inserted = total.inserted,
        skipped = total.skipped,
        dry_run = args.dry_run,
        "flow-pattern bootstrap finished"
    );

    Ok(())
}
