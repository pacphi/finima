//! Maintainer-only CLI that seeds the Tier 2 semantic store from
//! previously-categorized transactions already in the database, then
//! records a placeholder SONA state snapshot per portfolio.
//!
//! Phase 1 (ADR-012): this bin runs the bootstrap loop with
//! `LabeledExample::vector = None`. The Jaccard backend ingests happily;
//! the RuVector backend rejects vector-less examples (by design — Phase 1
//! is bring-your-own-vectors). In either case we still persist a small
//! Phase 1 marker on `portfolios.sona_state` so operators can confirm the
//! bin ran. Phase 2 will fill in real precomputed embeddings and store
//! `RuVectorEmbeddingStore::snapshot_sona_state` here.
//!
//! Usage:
//!
//! ```text
//! cargo run -p finima-api --bin bootstrap_tier2 -- \
//!     [--portfolio-id UUID] [--dry-run] [--limit N]
//! ```
//!
//! End users never run this.

#[path = "../config.rs"]
#[allow(dead_code)]
mod config;

use sqlx::postgres::PgPoolOptions;
use sqlx::Row;
use uuid::Uuid;

use finima_categorize::config::Tier2Backend;
use finima_categorize::tier2::{bootstrap_semantic, EmbeddingStore, LabeledExample};
use finima_categorize::CategorizeConfig;
use finima_db::PgPortfolioRepo;

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
                args.limit_override =
                    Some(raw.parse::<usize>().expect("--limit must be a non-negative integer"));
            }
            "--help" | "-h" => {
                println!(
                    "Usage: bootstrap_tier2 \
                     [--portfolio-id UUID] [--dry-run] [--limit N]"
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
    tracing_subscriber::fmt::init();

    let args = parse_args();
    let app_config = load_config()?;

    let pool = PgPoolOptions::new()
        .max_connections(app_config.database.max_connections)
        .connect(&app_config.database.resolved_url())
        .await?;

    let portfolio_repo = PgPortfolioRepo::new(pool.clone());

    let cat_cfg: CategorizeConfig = app_config.categorize.clone().into();
    let min_conf = cat_cfg.semantic_min_confidence;
    let backend = cat_cfg.tier2.resolved_backend();
    let max_examples = args
        .limit_override
        .unwrap_or(cat_cfg.tier2.bootstrap_max_examples);

    tracing::info!(
        backend = %backend.as_str(),
        min_confidence = min_conf,
        max_examples,
        dry_run = args.dry_run,
        "Tier 2 bootstrap starting"
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

    let mut total_offered = 0usize;
    let mut total_inserted = 0usize;

    for portfolio_id in &portfolio_ids {
        // Phase 1 bootstrap source: transactions with both category and
        // subcategory present. Per ADR-012 the corpus is hundreds to low
        // thousands per portfolio, so a single fetch is fine.
        let rows = sqlx::query(
            "SELECT t.description, t.category, t.subcategory, t.llm_confidence
               FROM transactions t
               JOIN accounts a ON a.id = t.account_id
              WHERE a.portfolio_id = $1
                AND t.category IS NOT NULL
                AND t.subcategory IS NOT NULL",
        )
        .bind(portfolio_id)
        .fetch_all(&pool)
        .await?;

        if rows.is_empty() {
            tracing::info!(
                portfolio_id = %portfolio_id,
                "no labeled transactions found; skipping"
            );
            continue;
        }

        let examples: Vec<LabeledExample> = rows
            .iter()
            .map(|row| LabeledExample {
                description: row.get::<String, _>("description"),
                category: row.get::<String, _>("category"),
                subcategory: row.get::<String, _>("subcategory"),
                confidence: row
                    .try_get::<Option<f64>, _>("llm_confidence")
                    .ok()
                    .flatten()
                    .unwrap_or(0.9),
                vector: None,
            })
            .collect();

        // Build a fresh Tier 2 store for this portfolio. The bootstrap
        // driver requires `SemanticVectorIngest`, so we match the backend
        // variant directly rather than going through
        // `CascadeEngine::build_semantic_from_config` (which erases to
        // `dyn SemanticCategorizer`).
        let offered = examples.len();
        let (report, err) = match backend {
            Tier2Backend::Jaccard => {
                let mut store = EmbeddingStore::new(min_conf);
                bootstrap_semantic(&mut store, examples, max_examples)
            }
            #[cfg(feature = "sona")]
            Tier2Backend::RuVector => {
                use finima_categorize::tier2::RuVectorEmbeddingStore;
                match RuVectorEmbeddingStore::new(cat_cfg.tier2.clone(), min_conf) {
                    Ok(mut store) => bootstrap_semantic(&mut store, examples, max_examples),
                    Err(e) => {
                        tracing::error!(
                            portfolio_id = %portfolio_id,
                            error = %e,
                            "failed to construct RuVector backend; skipping portfolio"
                        );
                        continue;
                    }
                }
            }
            #[cfg(not(feature = "sona"))]
            Tier2Backend::RuVector => unreachable!(
                "resolved_backend() must downgrade RuVector to Jaccard without the `sona` feature"
            ),
        };

        tracing::info!(
            portfolio_id = %portfolio_id,
            offered = report.offered,
            inserted = report.inserted,
            skipped_cap = report.skipped_cap,
            rejected = report.rejected,
            elapsed_ms = report.elapsed.as_millis() as u64,
            "bootstrap complete"
        );
        if let Some(e) = err {
            tracing::warn!(portfolio_id = %portfolio_id, error = %e, "bootstrap reported error");
        }

        total_offered += offered;
        total_inserted += report.inserted;

        if args.dry_run {
            continue;
        }

        // Phase 1 placeholder: store a small marker so operators can verify
        // the bin ran. Phase 2 will replace this with
        // `RuVectorEmbeddingStore::snapshot_sona_state`.
        let snapshot = serde_json::json!({
            "version": 1,
            "phase": "1",
            "backend": backend.as_str(),
            "report": {
                "offered": report.offered,
                "inserted": report.inserted,
                "skipped_cap": report.skipped_cap,
                "rejected": report.rejected,
                "elapsed_ms": report.elapsed.as_millis() as u64,
            },
            "timestamp": chrono::Utc::now().to_rfc3339(),
        });

        if let Err(e) = portfolio_repo.save_sona_state(*portfolio_id, &snapshot).await {
            tracing::error!(
                portfolio_id = %portfolio_id,
                error = %e,
                "failed to persist sona_state marker"
            );
        }
    }

    tracing::info!(
        portfolios = portfolio_ids.len(),
        offered = total_offered,
        inserted = total_inserted,
        dry_run = args.dry_run,
        "Tier 2 bootstrap finished"
    );

    Ok(())
}
