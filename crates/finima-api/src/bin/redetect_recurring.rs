//! Maintainer-only CLI to re-run recurring-transaction detection across
//! every portfolio (or a single one) and repopulate the
//! `recurring_groups` table.
//!
//! Recurring detection normally runs only as the final step of the
//! categorization pipeline (`handlers/categorization.rs`). Restarting the
//! server does NOT re-derive recurring groups — it re-serves whatever the
//! table already contains. After a classifier change (e.g. ADR-019) the
//! stored rows reflect the *old* algorithm until detection is re-triggered.
//!
//! This binary wipes all **unconfirmed** recurring groups for each
//! portfolio (user-confirmed groups are preserved) and re-upserts the
//! detector's output using the current configuration and code.
//!
//! Usage:
//!
//! ```text
//! cargo run -p finima-api --bin finima-redetect-recurring -- \
//!     [--portfolio-id UUID] [--dry-run]
//! ```
//!
//! End users never run this. See ADR-019.

#[path = "../config.rs"]
#[allow(dead_code)]
mod config;

use sqlx::postgres::PgPoolOptions;
use sqlx::Row;
use uuid::Uuid;

use finima_db::{PgRecurringRepo, PgTransactionRepo, RecurringGroupInsert};

use config::load_config;

#[derive(Debug, Default)]
struct Args {
    portfolio_id: Option<Uuid>,
    dry_run: bool,
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
            "--help" | "-h" => {
                println!(
                    "Usage: finima-redetect-recurring \
                     [--portfolio-id UUID] [--dry-run]"
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

    let transaction_repo = PgTransactionRepo::new(pool.clone());
    let recurring_repo = PgRecurringRepo::new(pool.clone());

    let detector_config = finima_analysis::RecurringDetectorConfig::from(config.recurring);

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
        println!("No portfolios found.");
        return Ok(());
    }

    println!(
        "Redetecting recurring groups across {} portfolio(s){}",
        portfolio_ids.len(),
        if args.dry_run { " (dry-run)" } else { "" }
    );
    println!("Config: {:?}", detector_config);

    let mut total_candidates = 0usize;
    let mut total_deleted = 0u64;

    for portfolio_id in &portfolio_ids {
        let rows = transaction_repo
            .list_for_analysis(*portfolio_id, None, None)
            .await?;

        if rows.is_empty() {
            println!("  portfolio {}: no transactions, skipping", portfolio_id);
            continue;
        }

        let analysis_txns: Vec<finima_analysis::TransactionForAnalysis> = rows
            .iter()
            .map(|t| finima_analysis::TransactionForAnalysis {
                id: t.id,
                date: t.date,
                amount: t.amount,
                description: t.description.clone(),
                merchant_name: t.merchant_name.clone(),
                category: t.category.clone(),
                account_id: Some(t.account_id),
            })
            .collect();

        let candidates =
            finima_analysis::detect_recurring_with_config(&analysis_txns, detector_config);

        println!(
            "  portfolio {}: {} transactions → {} candidates",
            portfolio_id,
            analysis_txns.len(),
            candidates.len()
        );

        if args.dry_run {
            for c in candidates.iter().take(20) {
                println!(
                    "    {:<40} {:<12} {:>10} (n={})",
                    c.merchant_name,
                    c.frequency.to_string(),
                    c.avg_amount.round_dp(2),
                    c.transaction_count
                );
            }
            if candidates.len() > 20 {
                println!("    ... and {} more", candidates.len() - 20);
            }
            total_candidates += candidates.len();
            continue;
        }

        let deleted = recurring_repo
            .delete_unconfirmed_by_portfolio(*portfolio_id)
            .await?;
        total_deleted += deleted;

        for candidate in &candidates {
            let insert = RecurringGroupInsert {
                merchant_name: candidate.merchant_name.clone(),
                category: candidate
                    .category
                    .clone()
                    .unwrap_or_else(|| "other".to_string()),
                frequency: candidate.frequency,
                avg_amount: candidate.avg_amount,
                next_expected_date: candidate.next_expected_date,
                metadata: serde_json::json!({
                    "transaction_count": candidate.transaction_count,
                    "annual_cost": candidate.annual_cost.to_string(),
                }),
            };
            recurring_repo.upsert(*portfolio_id, insert).await?;
        }

        total_candidates += candidates.len();
    }

    println!();
    if args.dry_run {
        println!(
            "Dry-run complete. {} candidate(s) would be written.",
            total_candidates
        );
    } else {
        println!(
            "Done. Deleted {} unconfirmed row(s); upserted {} candidate(s).",
            total_deleted, total_candidates
        );
    }

    Ok(())
}
