//! Merchant Audit CLI
//!
//! Connects to the Finima database, loads the seed merchant registry, and
//! produces a report showing uncategorized transactions, tier distribution,
//! and merchants that have been categorized by the LLM but are not yet in
//! the seed data. The suggested merchants are printed as JSON snippets that
//! can be appended to `seed_merchants.json`.

#[path = "../config.rs"]
mod config;

use finima_categorize::{MerchantRegistry, SEED_MERCHANTS_JSON};

/// A categorized merchant row from the database.
#[derive(Debug, sqlx::FromRow)]
struct CategorizedMerchant {
    merchant_name: String,
    category: String,
    subcategory: String,
    cnt: i64,
}

/// An uncategorized description row.
#[derive(Debug, sqlx::FromRow)]
struct UncategorizedDesc {
    description: String,
    cnt: i64,
}

/// Tier distribution row.
#[derive(Debug, sqlx::FromRow)]
struct TierRow {
    source_tier: Option<String>,
    cnt: i64,
}

#[tokio::main]
async fn main() {
    // Load .env if present, just like the main binary.
    dotenvy::dotenv().ok();

    // Load configuration using the same loader as the API server.
    let app_config = config::load_config().expect("Failed to load configuration");

    // Create database connection pool.
    let pool = finima_db::create_pool(
        &app_config.database.resolved_url(),
        app_config.database.max_connections,
    )
    .await
    .expect("Failed to create database pool");

    // Load the seed merchant registry.
    let mut registry = MerchantRegistry::with_defaults();
    let seed_count = registry
        .load_seed_merchants(SEED_MERCHANTS_JSON)
        .expect("Failed to load seed merchants");

    // ── Total / categorized / uncategorized counts ──

    let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM transactions")
        .fetch_one(&pool)
        .await
        .expect("Failed to count transactions");
    let total = total.0;

    let categorized: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM transactions WHERE category IS NOT NULL AND category != ''",
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to count categorized transactions");
    let categorized = categorized.0;

    let uncategorized = total - categorized;
    let pct = if total > 0 {
        (categorized as f64 / total as f64) * 100.0
    } else {
        0.0
    };

    // ── Tier distribution ──

    let tiers: Vec<TierRow> = sqlx::query_as(
        "SELECT source_tier, COUNT(*) as cnt \
         FROM transactions \
         WHERE category IS NOT NULL AND category != '' \
         GROUP BY source_tier \
         ORDER BY cnt DESC",
    )
    .fetch_all(&pool)
    .await
    .expect("Failed to query tier distribution");

    // ── Top uncategorized descriptions ──

    let uncat_descs: Vec<UncategorizedDesc> = sqlx::query_as(
        "SELECT description, COUNT(*) as cnt \
         FROM transactions \
         WHERE category IS NULL OR category = '' \
         GROUP BY description \
         ORDER BY cnt DESC \
         LIMIT 20",
    )
    .fetch_all(&pool)
    .await
    .expect("Failed to query uncategorized descriptions");

    // ── Suggested seed merchants (LLM-categorized, not in seed data) ──

    let llm_merchants: Vec<CategorizedMerchant> = sqlx::query_as(
        "SELECT merchant_name, category, subcategory, COUNT(*) as cnt \
         FROM transactions \
         WHERE source_tier = 'llm' \
           AND merchant_name IS NOT NULL \
           AND merchant_name != '' \
           AND category IS NOT NULL \
           AND category != '' \
         GROUP BY merchant_name, category, subcategory \
         ORDER BY cnt DESC",
    )
    .fetch_all(&pool)
    .await
    .expect("Failed to query LLM merchants");

    // Filter to merchants NOT already in the registry.
    let suggested: Vec<&CategorizedMerchant> = llm_merchants
        .iter()
        .filter(|m| registry.lookup(&m.merchant_name, None).is_none())
        .collect();

    // ── Print report ──

    println!("Merchant Audit Report");
    println!("=====================");
    println!();
    println!(
        "Transactions: {} total, {} categorized ({:.0}%), {} uncategorized",
        total, categorized, pct, uncategorized
    );
    println!();

    // Tier distribution
    if !tiers.is_empty() {
        println!("Tier Distribution:");
        for tier in &tiers {
            let label = tier
                .source_tier
                .as_deref()
                .unwrap_or("(none)");
            let tier_pct = if categorized > 0 {
                (tier.cnt as f64 / categorized as f64) * 100.0
            } else {
                0.0
            };
            println!("  {:<20} {:>5} ({:.0}%)", label, tier.cnt, tier_pct);
        }
        println!();
    }

    // Top uncategorized descriptions
    if !uncat_descs.is_empty() {
        println!("Top Uncategorized Descriptions:");
        for desc in &uncat_descs {
            println!("  {:>4}x  {}", desc.cnt, desc.description);
        }
        println!();
    }

    // Suggested seed merchants
    if !suggested.is_empty() {
        println!(
            "Suggested Seed Merchants (from LLM results, not in current seed data — {} seed entries loaded):",
            seed_count
        );
        for m in &suggested {
            let alias = m.merchant_name.to_uppercase();
            println!(
                "  {{\"name\": \"{}\", \"aliases\": [\"{}\"], \"category\": \"{}\", \"subcategory\": \"{}\"}},",
                m.merchant_name, alias, m.category, m.subcategory
            );
        }
        println!();
        println!("To add these, append them to:");
        println!("  crates/finima-categorize/data/seed_merchants.json");
    } else {
        println!("No suggested seed merchants — all LLM-categorized merchants are already in the seed data.");
    }
}
