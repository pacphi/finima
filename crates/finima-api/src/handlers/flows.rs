use std::collections::HashMap;

use axum::extract::{Path, Query, State};
use axum::response::IntoResponse;
use axum::Json;
use chrono::Datelike;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use finima_analysis::sona::FlowPattern;
use finima_analysis::{build_sankey_data, build_waterfall, detect_flows, FlowRecord};
use finima_auth::middleware::AuthUser;
use finima_core::traits::{AccountRepo, PortfolioRepo};
use finima_core::{next_month_start, AppError};
use finima_db::repos::NewFlowPattern;
use finima_db::NewAccountFlow;

use crate::state::AppState;

use super::helpers::{parse_month, resolve_portfolio_id, to_analysis};

// ---------------------------------------------------------------------------
// Request / response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct MonthQuery {
    pub month: Option<String>,
    pub portfolio_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct BalanceImpactQuery {
    pub month: Option<String>,
    pub account_id: Uuid,
    pub portfolio_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct PortfolioQuery {
    pub portfolio_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct CreateFlowRequest {
    pub source_transaction_id: Uuid,
    pub target_transaction_id: Uuid,
    pub portfolio_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct FlowActionRequest {
    /// "confirm" or "dismiss"
    pub action: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateFlowGroupRequest {
    pub name: String,
    pub portfolio_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateFlowGroupRequest {
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct SankeyResponse {
    pub nodes: Vec<SankeyNodeResponse>,
    pub links: Vec<SankeyLinkResponse>,
}

#[derive(Debug, Serialize)]
pub struct SankeyNodeResponse {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub node_type: String,
}

#[derive(Debug, Serialize)]
pub struct SankeyLinkResponse {
    pub source: String,
    pub target: String,
    pub value: Decimal,
}

#[derive(Debug, Serialize)]
pub struct OutflowRankResponse {
    pub account_id: Option<Uuid>,
    pub account_name: String,
    pub account_type: String,
    pub monthly_amount: Decimal,
    pub pct_income: f64,
    pub trend: String,
}

#[derive(Debug, Serialize)]
pub struct WaterfallResponse {
    pub start_balance: Decimal,
    pub income: Decimal,
    pub outflows: Vec<WaterfallOutflowResponse>,
    pub end_balance: Decimal,
}

#[derive(Debug, Serialize)]
pub struct WaterfallOutflowResponse {
    pub name: String,
    pub amount: Decimal,
}

#[derive(Debug, Serialize)]
pub struct FullSankeyResponse {
    pub nodes: Vec<FullSankeyNodeResponse>,
    pub links: Vec<SankeyLinkResponse>,
}

#[derive(Debug, Serialize)]
pub struct FullSankeyNodeResponse {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub node_type: String,
    /// "left" = income source, "middle" = account, "right" = spending category
    pub column: String,
}

/// Enriched flow record returned by GET /api/flows. Includes human-readable
/// account names and transaction descriptions so the UI can display meaningful
/// information instead of raw UUIDs.
#[derive(Debug, Serialize)]
pub struct FlowDetailResponse {
    pub id: Uuid,
    pub source_account_id: Uuid,
    pub source_account_name: String,
    #[serde(rename = "destination_account_id")]
    pub target_account_id: Uuid,
    #[serde(rename = "destination_account_name")]
    pub target_account_name: String,
    pub source_transaction_id: Option<Uuid>,
    pub source_description: Option<String>,
    #[serde(rename = "destination_transaction_id")]
    pub target_transaction_id: Option<Uuid>,
    #[serde(rename = "destination_description")]
    pub target_description: Option<String>,
    pub amount: Decimal,
    #[serde(rename = "date")]
    pub flow_date: chrono::NaiveDate,
    pub is_auto_detected: bool,
    pub is_confirmed: bool,
}

// ---------------------------------------------------------------------------
// Flow handlers
// ---------------------------------------------------------------------------

/// GET /api/flows?month=2026-04
pub async fn list_flows(
    user: AuthUser,
    State(state): State<AppState>,
    Query(params): Query<MonthQuery>,
) -> Result<impl IntoResponse, AppError> {
    let portfolio_id = resolve_portfolio_id(&user, &state, params.portfolio_id).await?;
    let month = parse_month(&params.month)?;

    let flows = state
        .flow_repo()
        .list_by_portfolio_month(portfolio_id, month)
        .await?;

    // Build account name lookup.
    let accounts = state.account_repo().list_by_portfolio(portfolio_id).await?;
    let acct_names: HashMap<Uuid, String> =
        accounts.iter().map(|a| (a.id, a.name.clone())).collect();

    // Collect all referenced transaction IDs for a single batch lookup.
    let txn_ids: Vec<Uuid> = flows
        .iter()
        .flat_map(|f| {
            f.source_transaction_id
                .into_iter()
                .chain(f.target_transaction_id)
        })
        .collect();

    // Fetch transaction descriptions. Use a simple HashMap keyed by ID.
    let mut txn_descs: HashMap<Uuid, String> = HashMap::new();
    for txn_id in &txn_ids {
        if let Ok(txn) = state.transaction_repo().find_by_id(*txn_id).await {
            txn_descs.insert(txn.id, txn.description.clone());
        }
    }

    let enriched: Vec<FlowDetailResponse> = flows
        .into_iter()
        .map(|f| FlowDetailResponse {
            id: f.id,
            source_account_id: f.source_account_id,
            source_account_name: acct_names
                .get(&f.source_account_id)
                .cloned()
                .unwrap_or_else(|| "Unknown".into()),
            target_account_id: f.target_account_id,
            target_account_name: acct_names
                .get(&f.target_account_id)
                .cloned()
                .unwrap_or_else(|| "Unknown".into()),
            source_transaction_id: f.source_transaction_id,
            source_description: f
                .source_transaction_id
                .and_then(|id| txn_descs.get(&id).cloned()),
            target_transaction_id: f.target_transaction_id,
            target_description: f
                .target_transaction_id
                .and_then(|id| txn_descs.get(&id).cloned()),
            amount: f.amount,
            flow_date: f.flow_date,
            is_auto_detected: f.is_auto_detected,
            is_confirmed: f.is_confirmed,
        })
        .collect();

    Ok(Json(enriched))
}

/// POST /api/flows — manually link two transactions as a transfer pair.
pub async fn create_flow(
    user: AuthUser,
    State(state): State<AppState>,
    Json(body): Json<CreateFlowRequest>,
) -> Result<impl IntoResponse, AppError> {
    let portfolio_id = resolve_portfolio_id(&user, &state, body.portfolio_id).await?;

    // Fetch both transactions to determine accounts and amount.
    let source_txn = state
        .transaction_repo()
        .find_by_id(body.source_transaction_id)
        .await?;
    let target_txn = state
        .transaction_repo()
        .find_by_id(body.target_transaction_id)
        .await?;

    let new_flow = NewAccountFlow {
        portfolio_id,
        source_account_id: source_txn.account_id,
        target_account_id: target_txn.account_id,
        source_transaction_id: Some(source_txn.id),
        target_transaction_id: Some(target_txn.id),
        amount: source_txn.amount.abs(),
        flow_date: source_txn.date,
        is_auto_detected: false,
    };

    let flow = state.flow_repo().create(&new_flow).await?;

    Ok(Json(flow))
}

/// PUT /api/flows/:id — confirm or dismiss a flow.
pub async fn update_flow(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<FlowActionRequest>,
) -> Result<impl IntoResponse, AppError> {
    // Verify ownership: flow -> portfolio -> user
    let flow = state.flow_repo().find_by_id(id).await?;
    state
        .portfolio_repo()
        .verify_ownership(flow.portfolio_id, user.user_id)
        .await?;

    // Look up the source transaction description up-front so both
    // confirm and dismiss branches can feed the pattern store.
    // Errors here must NOT fail the handler — the user's confirm/dismiss
    // action is the primary signal and already authorized.
    let source_desc: Option<String> = match flow.source_transaction_id {
        Some(txn_id) => match state.transaction_repo().find_by_id(txn_id).await {
            Ok(txn) => Some(txn.description),
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    flow_id = %id,
                    "failed to load source transaction for flow-pattern write; skipping"
                );
                None
            }
        },
        None => None,
    };

    match body.action.as_str() {
        "confirm" => {
            state.flow_repo().confirm(id).await?;

            if let Some(desc) = source_desc.as_ref() {
                // Best-effort embedding for the pattern row. When the
                // embedder is noop or fails, fall through with no vector
                // — the Jaccard / Stub paths work fine without it.
                let embedder = state.embedder();
                let (embedding_bytes, embedding_dim, embedding_vec): (
                    Option<Vec<u8>>,
                    Option<i32>,
                    Option<Vec<f32>>,
                ) = if embedder.backend() != "noop" {
                    match embedder.embed(desc).await {
                        Ok(v) => {
                            let bytes = v.iter().flat_map(|f| f.to_le_bytes()).collect::<Vec<u8>>();
                            let dim = v.len() as i32;
                            (Some(bytes), Some(dim), Some(v))
                        }
                        Err(e) => {
                            tracing::debug!(
                                error = %e,
                                flow_id = %id,
                                "embedder.embed failed on confirm; persisting pattern without vector"
                            );
                            (None, None, None)
                        }
                    }
                } else {
                    (None, None, None)
                };

                let new_pattern = NewFlowPattern {
                    portfolio_id: flow.portfolio_id,
                    description_text: desc.clone(),
                    source_account_id: flow.source_account_id,
                    target_account_id: flow.target_account_id,
                    confidence: 1.0,
                    embedding: embedding_bytes,
                    embedding_dim,
                };
                if let Err(e) = state
                    .flow_pattern_repo()
                    .upsert_confirmed(new_pattern)
                    .await
                {
                    tracing::warn!(
                        error = %e,
                        flow_id = %id,
                        "flow_patterns upsert_confirmed failed; continuing"
                    );
                } else if let Some(m) = state.metrics() {
                    m.metrics().flow_patterns_confirmed_total.inc();
                }

                // In-memory matcher update. When we have both a concrete
                // RuVector matcher AND a vector, prefer the vector path
                // so the HNSW index learns the pattern. Otherwise fall
                // through to the trait-level `store_pattern` (no-op on
                // RuVector, meaningful on Stub).
                #[cfg(feature = "sona")]
                {
                    if let (Some(vec), Some(rv)) =
                        (embedding_vec.as_ref(), state.flow_matcher_ruvector())
                    {
                        if let Ok(mut matcher) = rv.write() {
                            matcher.store_pattern_with_vector(
                                FlowPattern {
                                    description: desc.clone(),
                                    source_account_id: flow.source_account_id,
                                    target_account_id: flow.target_account_id,
                                    confidence: 1.0,
                                    match_count: 1,
                                },
                                vec,
                            );
                        }
                    } else if let Ok(mut matcher) = state.flow_matcher().write() {
                        matcher.store_pattern(FlowPattern {
                            description: desc.clone(),
                            source_account_id: flow.source_account_id,
                            target_account_id: flow.target_account_id,
                            confidence: 1.0,
                            match_count: 1,
                        });
                    }
                }
                #[cfg(not(feature = "sona"))]
                {
                    let _ = embedding_vec; // feature-gated consumer
                    if let Ok(mut matcher) = state.flow_matcher().write() {
                        matcher.store_pattern(FlowPattern {
                            description: desc.clone(),
                            source_account_id: flow.source_account_id,
                            target_account_id: flow.target_account_id,
                            confidence: 1.0,
                            match_count: 1,
                        });
                    }
                }
            }

            Ok(Json(serde_json::json!({"status": "confirmed"})))
        }
        "dismiss" => {
            state.flow_repo().dismiss(id).await?;

            if let Some(desc) = source_desc.as_ref() {
                if let Err(e) = state
                    .flow_pattern_repo()
                    .record_dismissal(flow.portfolio_id, flow.source_account_id, desc)
                    .await
                {
                    tracing::warn!(
                        error = %e,
                        flow_id = %id,
                        "flow_patterns record_dismissal failed; continuing"
                    );
                } else if let Some(m) = state.metrics() {
                    m.metrics().flow_patterns_dismissed_total.inc();
                }

                if let Ok(mut matcher) = state.flow_matcher().write() {
                    matcher.record_dismissal(desc, flow.source_account_id);
                }
            }

            Ok(Json(serde_json::json!({"status": "dismissed"})))
        }
        _ => Err(AppError::BadRequest(
            "action must be 'confirm' or 'dismiss'".to_string(),
        )),
    }
}

/// DELETE /api/flows/:id
pub async fn delete_flow(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    // Verify ownership: flow -> portfolio -> user
    let flow = state.flow_repo().find_by_id(id).await?;
    state
        .portfolio_repo()
        .verify_ownership(flow.portfolio_id, user.user_id)
        .await?;

    state.flow_repo().delete(id).await?;

    Ok(Json(serde_json::json!({"status": "deleted"})))
}

/// GET /api/flows/sankey?month=2026-04
pub async fn get_sankey(
    user: AuthUser,
    State(state): State<AppState>,
    Query(params): Query<MonthQuery>,
) -> Result<impl IntoResponse, AppError> {
    let portfolio_id = resolve_portfolio_id(&user, &state, params.portfolio_id).await?;
    let month = parse_month(&params.month)?;

    let db_flows = state
        .flow_repo()
        .list_by_portfolio_month(portfolio_id, month)
        .await?;

    // Build account name lookup.
    let accounts = state.account_repo().list_by_portfolio(portfolio_id).await?;
    let acct_names: HashMap<Uuid, String> =
        accounts.iter().map(|a| (a.id, a.name.clone())).collect();

    // Convert DB flows to analysis FlowRecords.
    let flow_records: Vec<FlowRecord> = db_flows
        .iter()
        .map(|f| FlowRecord {
            source_account_id: f.source_account_id,
            source_account_name: acct_names
                .get(&f.source_account_id)
                .cloned()
                .unwrap_or_else(|| "Unknown".to_string()),
            target_account_id: Some(f.target_account_id),
            target_account_name: acct_names.get(&f.target_account_id).cloned(),
            amount: f.amount,
            flow_date: f.flow_date,
            category: None,
        })
        .collect();

    let sankey = build_sankey_data(&flow_records, month);

    // Determine which node ids appear as sources vs targets in links.
    let source_ids: std::collections::HashSet<&str> =
        sankey.links.iter().map(|l| l.source.as_str()).collect();

    let response = SankeyResponse {
        nodes: sankey
            .nodes
            .into_iter()
            .map(|n| {
                let node_type = if source_ids.contains(n.id.as_str()) {
                    "source".to_string()
                } else {
                    "target".to_string()
                };
                SankeyNodeResponse {
                    id: n.id,
                    name: n.label,
                    node_type,
                }
            })
            .collect(),
        links: sankey
            .links
            .into_iter()
            .map(|l| SankeyLinkResponse {
                source: l.source,
                target: l.target,
                value: l.value,
            })
            .collect(),
    };

    Ok(Json(response))
}

/// GET /api/flows/sankey-full?month=2026-04
///
/// Returns a three-column Sankey: income sources → accounts → spending categories.
/// Inter-account transfers (from account_flows) form the middle links.
/// Income and spending links are derived from transactions, excluding those
/// already accounted for as inter-account transfers.
pub async fn get_full_sankey(
    user: AuthUser,
    State(state): State<AppState>,
    Query(params): Query<MonthQuery>,
) -> Result<impl IntoResponse, AppError> {
    let portfolio_id = resolve_portfolio_id(&user, &state, params.portfolio_id).await?;
    let month = parse_month(&params.month)?;

    let db_flows = state
        .flow_repo()
        .list_by_portfolio_month(portfolio_id, month)
        .await?;
    let accounts = state.account_repo().list_by_portfolio(portfolio_id).await?;

    let acct_names: HashMap<Uuid, String> =
        accounts.iter().map(|a| (a.id, a.name.clone())).collect();
    let primary_ids: std::collections::HashSet<Uuid> = accounts
        .iter()
        .filter(|a| a.is_primary_income)
        .map(|a| a.id)
        .collect();

    // Load transactions for the month (half-open `[month, next_month_start)`).
    let end_of_month = next_month_start(month);
    let txn_rows = state
        .transaction_repo()
        .list_for_analysis(portfolio_id, Some(month), Some(end_of_month))
        .await?;

    // Transaction IDs already represented in account_flows (transfers).
    let transfer_txn_ids: std::collections::HashSet<Uuid> = db_flows
        .iter()
        .flat_map(|f| {
            f.source_transaction_id
                .into_iter()
                .chain(f.target_transaction_id)
        })
        .collect();

    // ---- Inter-account transfer links (middle) ----
    let mut transfer_links: HashMap<(String, String), Decimal> = HashMap::new();
    for f in &db_flows {
        let src = acct_names
            .get(&f.source_account_id)
            .cloned()
            .unwrap_or_else(|| "Unknown".into());
        let tgt = acct_names
            .get(&f.target_account_id)
            .cloned()
            .unwrap_or_else(|| "Unknown".into());
        *transfer_links.entry((src, tgt)).or_default() += f.amount;
    }

    // ---- Income links (left → middle) ----
    let mut income_links: HashMap<(String, String), Decimal> = HashMap::new();
    for row in &txn_rows {
        if row.amount <= Decimal::ZERO {
            continue;
        }
        if transfer_txn_ids.contains(&row.id) {
            continue;
        }
        if !primary_ids.contains(&row.account_id) {
            continue;
        }
        if row.category.as_deref() == Some("transfer") {
            continue;
        }

        let source_label = row
            .category
            .as_ref()
            .filter(|c| c.as_str() != "transfer" && c.as_str() != "uncategorized")
            .map(|c| finima_llm::titlecase(c))
            .or_else(|| row.merchant_name.clone())
            .unwrap_or_else(|| "Other Income".into());
        let target_label = acct_names
            .get(&row.account_id)
            .cloned()
            .unwrap_or_else(|| "Unknown".into());
        *income_links
            .entry((source_label, target_label))
            .or_default() += row.amount;
    }

    // ---- Spending links (middle → right) ----
    //
    // Direction is normalized at import time by SignNormalizer
    // (see ADR-018). This block queries `direction == Outflow` and is
    // free of institution-specific or account-type sign conditionals.
    //
    // Categories listed in `sankey.transfer_categories` are excluded
    // because they represent transfers between accounts (already
    // captured as primary→secondary transfer links above) rather than
    // consumption.
    use finima_core::TransactionDirection;
    let transfer_categories: std::collections::HashSet<&str> = state
        .config()
        .sankey
        .transfer_categories
        .iter()
        .map(String::as_str)
        .collect();

    let mut spending_links: HashMap<(String, String), Decimal> = HashMap::new();
    for row in &txn_rows {
        if transfer_txn_ids.contains(&row.id) {
            continue;
        }
        // Skip rows whose direction has not been normalized yet
        // (legacy NULL rows). Run `finima-normalize-directions` to
        // populate them.
        if row.direction != Some(TransactionDirection::Outflow) {
            continue;
        }
        if let Some(cat) = row.category.as_deref() {
            if transfer_categories.contains(cat) {
                continue;
            }
        }

        let source_label = acct_names
            .get(&row.account_id)
            .cloned()
            .unwrap_or_else(|| "Unknown".into());
        let target_label = row
            .category
            .as_ref()
            .map(|c| finima_llm::titlecase(c))
            .unwrap_or_else(|| "Uncategorized".into());
        *spending_links
            .entry((source_label, target_label))
            .or_default() += row.amount.abs();
    }

    // ---- Apply 2% aggregation thresholds ----
    let total_income: Decimal = income_links.values().copied().sum();
    let income_threshold = total_income * Decimal::new(2, 2);

    let total_spending: Decimal = spending_links.values().copied().sum();
    let spending_threshold = total_spending * Decimal::new(2, 2);

    // Aggregate small income sources into "Other Income".
    {
        let mut other_by_target: HashMap<String, Decimal> = HashMap::new();
        let orig = std::mem::take(&mut income_links);
        for ((src, tgt), val) in orig {
            if val < income_threshold {
                *other_by_target.entry(tgt).or_default() += val;
            } else {
                income_links.insert((src, tgt), val);
            }
        }
        for (tgt, val) in other_by_target {
            if val > Decimal::ZERO {
                *income_links
                    .entry(("Other Income".into(), tgt))
                    .or_default() += val;
            }
        }
    }

    // Aggregate small spending categories into "Other".
    {
        let mut other_by_source: HashMap<String, Decimal> = HashMap::new();
        let orig = std::mem::take(&mut spending_links);
        for ((src, tgt), val) in orig {
            if val < spending_threshold {
                *other_by_source.entry(src).or_default() += val;
            } else {
                spending_links.insert((src, tgt), val);
            }
        }
        for (src, val) in other_by_source {
            if val > Decimal::ZERO {
                *spending_links.entry((src, "Other".into())).or_default() += val;
            }
        }
    }

    // ---- Insert spender-role virtual nodes for primary direct spending ----
    //
    // To keep the 4-column layout strictly unidirectional (every link
    // goes col N → col N+1), we use the textbook Sugiyama dummy-node
    // technique: when a primary account has direct spending (debit
    // card, autopay), we synthesize a "{Primary} — Direct Debit"
    // node in column 2 and route the spending through it.
    //
    // Result:
    //   primary (col 1) → "{Primary} — Direct Debit" (col 2) → category (col 3)
    //
    // Without this, primary→category edges skip column 2 visually,
    // which produced the layout problems we hit in earlier
    // experiments. See ADR-008 Amendment 2 for the full rationale.
    let primary_names: std::collections::HashSet<String> = accounts
        .iter()
        .filter(|a| primary_ids.contains(&a.id))
        .map(|a| a.name.clone())
        .collect();

    let direct_debit_label = |primary: &str| format!("{} — Direct Debit", primary);

    type SpendingLinks = HashMap<(String, String), Decimal>;
    let (primary_spending, secondary_spending): (SpendingLinks, SpendingLinks) = spending_links
        .into_iter()
        .partition(|((src, _), _)| primary_names.contains(src));

    // For each primary→category link, emit two replacement links:
    //   primary (col 1) → spender_role (col 2)
    //   spender_role (col 2) → category (col 3)
    let mut spender_role_inflows: HashMap<(String, String), Decimal> = HashMap::new();
    let mut spender_role_spending: HashMap<(String, String), Decimal> = HashMap::new();
    for ((primary, category), amount) in primary_spending {
        let spender = direct_debit_label(&primary);
        *spender_role_inflows
            .entry((primary, spender.clone()))
            .or_default() += amount;
        *spender_role_spending
            .entry((spender, category))
            .or_default() += amount;
    }

    // ---- Collect nodes with column tags ----
    // 4-column layout so all flows go strictly left-to-right:
    //   col 0 ("left")      = income sources
    //   col 1 ("primary")   = primary account(s) — hub that receives income & sends transfers
    //   col 2 ("secondary") = non-primary accounts (credit cards, savings, etc.)
    //                         + spender-role virtual nodes for primary direct spending
    //   col 3 ("right")     = spending categories
    let mut nodes: Vec<FullSankeyNodeResponse> = Vec::new();
    let mut seen_nodes: std::collections::HashSet<String> = std::collections::HashSet::new();

    // Income source nodes (col 0).
    for (src, _tgt) in income_links.keys() {
        if seen_nodes.insert(src.clone()) {
            nodes.push(FullSankeyNodeResponse {
                id: src.clone(),
                name: src.clone(),
                node_type: "income".into(),
                column: "left".into(),
            });
        }
    }

    // Account nodes — split primary (col 1) vs non-primary (col 2).
    for account in &accounts {
        if seen_nodes.insert(account.name.clone()) {
            let col = if primary_ids.contains(&account.id) {
                "primary"
            } else {
                "secondary"
            };
            nodes.push(FullSankeyNodeResponse {
                id: account.name.clone(),
                name: account.name.clone(),
                node_type: "account".into(),
                column: col.into(),
            });
        }
    }

    // Spender-role virtual nodes (col 2) — one per primary that had
    // direct spending. Marked with node_type="spender_role" so the
    // frontend can render them with a distinguishing visual treatment.
    for (_primary, spender) in spender_role_inflows.keys() {
        if seen_nodes.insert(spender.clone()) {
            nodes.push(FullSankeyNodeResponse {
                id: spender.clone(),
                name: spender.clone(),
                node_type: "spender_role".into(),
                column: "secondary".into(),
            });
        }
    }

    // Spending category nodes (col 3).
    for (_src, tgt) in secondary_spending
        .keys()
        .chain(spender_role_spending.keys())
    {
        if seen_nodes.insert(tgt.clone()) {
            nodes.push(FullSankeyNodeResponse {
                id: tgt.clone(),
                name: tgt.clone(),
                node_type: "spending".into(),
                column: "right".into(),
            });
        }
    }

    // ---- Collect all links ----
    let mut links: Vec<SankeyLinkResponse> = Vec::new();

    for ((src, tgt), val) in &income_links {
        links.push(SankeyLinkResponse {
            source: src.clone(),
            target: tgt.clone(),
            value: *val,
        });
    }
    for ((src, tgt), val) in &transfer_links {
        links.push(SankeyLinkResponse {
            source: src.clone(),
            target: tgt.clone(),
            value: *val,
        });
    }
    // Primary → spender-role inflows (col 1 → col 2).
    for ((src, tgt), val) in &spender_role_inflows {
        links.push(SankeyLinkResponse {
            source: src.clone(),
            target: tgt.clone(),
            value: *val,
        });
    }
    // Spender-role → category outflows (col 2 → col 3).
    for ((src, tgt), val) in &spender_role_spending {
        links.push(SankeyLinkResponse {
            source: src.clone(),
            target: tgt.clone(),
            value: *val,
        });
    }
    // Secondary account → category outflows (col 2 → col 3).
    for ((src, tgt), val) in &secondary_spending {
        links.push(SankeyLinkResponse {
            source: src.clone(),
            target: tgt.clone(),
            value: *val,
        });
    }

    Ok(Json(FullSankeyResponse { nodes, links }))
}

/// GET /api/flows/outflow-ranking?month=2026-04
///
/// Ranks every primary-account outflow by monthly amount. Each row
/// is either:
///   - a **transfer** to a secondary account (type = account_type), or
///   - a **direct category** spend from a primary account
///     (type = "category").
///
/// This mirrors the edges leaving primary nodes in the Money Flow
/// Sankey so the ranking and the diagram tell the same story. See
/// ADR-008 Amendment 2.
pub async fn get_outflow_ranking(
    user: AuthUser,
    State(state): State<AppState>,
    Query(params): Query<MonthQuery>,
) -> Result<impl IntoResponse, AppError> {
    let portfolio_id = resolve_portfolio_id(&user, &state, params.portfolio_id).await?;
    let month = parse_month(&params.month)?;

    let db_flows = state
        .flow_repo()
        .list_by_portfolio_month(portfolio_id, month)
        .await?;

    let accounts = state.account_repo().list_by_portfolio(portfolio_id).await?;
    let primary_ids: std::collections::HashSet<Uuid> = accounts
        .iter()
        .filter(|a| a.is_primary_income)
        .map(|a| a.id)
        .collect();

    // Load transactions for the month (used for income total + direct spending).
    let end_of_month = next_month_start(month);
    let txn_rows = state
        .transaction_repo()
        .list_for_analysis(portfolio_id, Some(month), Some(end_of_month))
        .await?;

    // Total income: inflow on primary accounts (post-normalization).
    // Falls back to "positive amounts" for legacy NULL-direction rows.
    use finima_core::TransactionDirection;
    let total_income: Decimal = txn_rows
        .iter()
        .filter(|t| primary_ids.contains(&t.account_id))
        .filter(|t| match t.direction {
            Some(TransactionDirection::Inflow) => true,
            Some(TransactionDirection::Outflow) => false,
            None => t.amount > Decimal::ZERO,
        })
        .map(|t| t.amount.abs())
        .sum();

    // ─── Transfer rows: primary → secondary ───────────────────────
    let mut rows: Vec<OutflowRankResponse> = Vec::new();
    let mut transfer_totals: HashMap<Uuid, Decimal> = HashMap::new();
    for f in &db_flows {
        if primary_ids.contains(&f.source_account_id) && !primary_ids.contains(&f.target_account_id)
        {
            *transfer_totals.entry(f.target_account_id).or_default() += f.amount;
        }
    }
    for (account_id, amount) in transfer_totals {
        let account = accounts.iter().find(|a| a.id == account_id);
        let (account_name, account_type) = match account {
            Some(a) => (a.name.clone(), a.account_type.to_string()),
            None => ("Unknown".to_string(), "unknown".to_string()),
        };
        let pct_income = if total_income > Decimal::ZERO {
            (amount / total_income)
                .to_string()
                .parse::<f64>()
                .unwrap_or(0.0)
                * 100.0
        } else {
            0.0
        };
        rows.push(OutflowRankResponse {
            account_id: Some(account_id),
            account_name,
            account_type,
            monthly_amount: amount,
            pct_income,
            trend: "stable".to_string(),
        });
    }

    // ─── Direct category rows: primary → category (debit, autopay) ──
    let transfer_txn_ids: std::collections::HashSet<Uuid> = db_flows
        .iter()
        .flat_map(|f| {
            f.source_transaction_id
                .into_iter()
                .chain(f.target_transaction_id)
        })
        .collect();
    let transfer_categories: std::collections::HashSet<&str> = state
        .config()
        .sankey
        .transfer_categories
        .iter()
        .map(String::as_str)
        .collect();

    let mut category_totals: HashMap<String, Decimal> = HashMap::new();
    for t in &txn_rows {
        if !primary_ids.contains(&t.account_id) {
            continue;
        }
        if transfer_txn_ids.contains(&t.id) {
            continue;
        }
        if t.direction != Some(TransactionDirection::Outflow) {
            continue;
        }
        if let Some(cat) = t.category.as_deref() {
            if transfer_categories.contains(cat) {
                continue;
            }
        }
        let label = t
            .category
            .as_ref()
            .map(|c| finima_llm::titlecase(c))
            .unwrap_or_else(|| "Uncategorized".to_string());
        *category_totals.entry(label).or_default() += t.amount.abs();
    }
    for (category_label, amount) in category_totals {
        let pct_income = if total_income > Decimal::ZERO {
            (amount / total_income)
                .to_string()
                .parse::<f64>()
                .unwrap_or(0.0)
                * 100.0
        } else {
            0.0
        };
        rows.push(OutflowRankResponse {
            account_id: None,
            account_name: category_label,
            account_type: "category".to_string(),
            monthly_amount: amount,
            pct_income,
            trend: "stable".to_string(),
        });
    }

    // Sort by monthly amount, descending.
    rows.sort_by_key(|r| std::cmp::Reverse(r.monthly_amount));
    let response = rows;

    Ok(Json(response))
}

/// GET /api/flows/balance-impact?month=2026-04&account_id=uuid
pub async fn get_balance_impact(
    user: AuthUser,
    State(state): State<AppState>,
    Query(params): Query<BalanceImpactQuery>,
) -> Result<impl IntoResponse, AppError> {
    let portfolio_id = resolve_portfolio_id(&user, &state, params.portfolio_id).await?;
    let month = parse_month(&params.month)?;

    let account = state.account_repo().find_by_id(params.account_id).await?;

    let db_flows = state
        .flow_repo()
        .list_by_portfolio_month(portfolio_id, month)
        .await?;

    let accounts = state.account_repo().list_by_portfolio(portfolio_id).await?;
    let acct_names: HashMap<Uuid, String> =
        accounts.iter().map(|a| (a.id, a.name.clone())).collect();

    // Get transactions for the account to compute income for this month.
    let txn_rows = state
        .transaction_repo()
        .list_by_account_for_analysis(params.account_id)
        .await?;
    let txns = to_analysis(&txn_rows);

    // Sum income for the month.
    let month_income: Decimal = txns
        .iter()
        .filter(|t| {
            t.date.year() == month.year()
                && t.date.month() == month.month()
                && t.amount > Decimal::ZERO
        })
        .map(|t| t.amount)
        .sum();

    // Build outflows from flows where this account is the source.
    let outflows: Vec<(String, Decimal)> = db_flows
        .iter()
        .filter(|f| f.source_account_id == params.account_id)
        .map(|f| {
            let label = acct_names
                .get(&f.target_account_id)
                .cloned()
                .unwrap_or_else(|| "Unknown".to_string());
            (label, f.amount)
        })
        .collect();

    let start_balance = account.opening_balance;
    let waterfall = build_waterfall(params.account_id, start_balance, month_income, &outflows);

    // Map to the shape the frontend expects.
    let response = WaterfallResponse {
        start_balance: waterfall.start_balance,
        income: month_income,
        outflows: outflows
            .into_iter()
            .map(|(name, amount)| WaterfallOutflowResponse { name, amount })
            .collect(),
        end_balance: waterfall.end_balance,
    };

    Ok(Json(response))
}

/// POST /api/flows/detect?month=2026-04
///
/// Run flow detection for the given month. Matches outflows from primary income
/// accounts to inflows in other accounts using the ±1% amount / ±2 day window
/// heuristic. Skips flows that already exist in the database.
pub async fn detect_flows_handler(
    user: AuthUser,
    State(state): State<AppState>,
    Query(params): Query<MonthQuery>,
) -> Result<impl IntoResponse, AppError> {
    let portfolio_id = resolve_portfolio_id(&user, &state, params.portfolio_id).await?;
    let month = parse_month(&params.month)?;

    // Load all accounts, identify primary ones.
    let accounts = state.account_repo().list_by_portfolio(portfolio_id).await?;
    let primary_ids: Vec<Uuid> = accounts
        .iter()
        .filter(|a| a.is_primary_income)
        .map(|a| a.id)
        .collect();

    if primary_ids.is_empty() {
        return Err(AppError::BadRequest(
            "No primary income account configured. Set one on the Accounts page first.".to_string(),
        ));
    }

    // Fetch transactions for the month with a 2-day buffer on each side
    // for matching transfer pairs across the boundary. `end` is exclusive,
    // so `next_month_start + 2` keeps 2 days of the next month in scope.
    let start = month - chrono::Duration::days(2);
    let end = next_month_start(month) + chrono::Duration::days(2);

    let txn_rows = state
        .transaction_repo()
        .list_for_analysis(portfolio_id, Some(start), Some(end))
        .await?;
    let txns = to_analysis(&txn_rows);

    // Group by account_id.
    let mut by_account: HashMap<Uuid, Vec<finima_analysis::TransactionForAnalysis>> =
        HashMap::new();
    for txn in txns {
        if let Some(acct_id) = txn.account_id {
            by_account.entry(acct_id).or_default().push(txn);
        }
    }

    let candidates = detect_flows(&primary_ids, &by_account);

    // Build a lookup from keywords in account names to account IDs, so we can
    // resolve one-sided flows (e.g., "AMEX EPAYMENT" → the Amex card account).
    // Each word ≥3 chars from non-primary account names becomes a match key.
    let non_primary: Vec<(Uuid, String)> = accounts
        .iter()
        .filter(|a| !a.is_primary_income)
        .map(|a| (a.id, a.name.clone()))
        .collect();

    let resolve_target_from_description = |description: &str| -> Option<Uuid> {
        let upper = description.to_uppercase();
        for (acct_id, name) in &non_primary {
            // Match any word ≥3 chars from the account name against the description.
            for word in name.split_whitespace() {
                if word.len() >= 3 && upper.contains(&word.to_uppercase()) {
                    return Some(*acct_id);
                }
            }
        }
        None
    };

    // Also scan ALL outflows from primary accounts for transfer-like descriptions,
    // even those the heuristic matcher missed (no matching inflow). This catches
    // credit card payments where the card side doesn't show a corresponding inflow.
    let mut extra_candidates: Vec<(Uuid, Uuid, Uuid, Decimal, chrono::NaiveDate)> = Vec::new();
    let heuristic_source_txns: std::collections::HashSet<Uuid> =
        candidates.iter().map(|c| c.source_transaction_id).collect();

    for primary_id in &primary_ids {
        if let Some(txns) = by_account.get(primary_id) {
            for txn in txns {
                if txn.amount >= Decimal::ZERO {
                    continue;
                }
                if heuristic_source_txns.contains(&txn.id) {
                    continue; // already handled by heuristic
                }
                // Try to resolve target from description keywords.
                if let Some(target_id) = resolve_target_from_description(&txn.description) {
                    extra_candidates.push((
                        *primary_id,
                        target_id,
                        txn.id,
                        txn.amount.abs(),
                        txn.date,
                    ));
                }
            }
        }
    }

    // Load existing flows for the month to avoid duplicates.
    let existing = state
        .flow_repo()
        .list_by_portfolio_month(portfolio_id, month)
        .await?;

    let existing_source_txns: std::collections::HashSet<Uuid> = existing
        .iter()
        .filter_map(|f| f.source_transaction_id)
        .collect();

    let mut created = Vec::new();

    // Process heuristic candidates.
    for candidate in &candidates {
        if existing_source_txns.contains(&candidate.source_transaction_id) {
            continue;
        }

        // For one-sided flows, try to resolve the target from description.
        let target_account_id = candidate.target_account_id.or_else(|| {
            let desc = by_account
                .values()
                .flat_map(|txns| txns.iter())
                .find(|t| t.id == candidate.source_transaction_id)
                .map(|t| t.description.as_str())
                .unwrap_or("");
            resolve_target_from_description(desc)
        });

        let Some(target_account_id) = target_account_id else {
            continue;
        };

        let new_flow = NewAccountFlow {
            portfolio_id,
            source_account_id: candidate.source_account_id,
            target_account_id,
            source_transaction_id: Some(candidate.source_transaction_id),
            target_transaction_id: candidate.target_transaction_id,
            amount: candidate.amount,
            flow_date: candidate.flow_date,
            is_auto_detected: true,
        };

        if let Ok(flow) = state.flow_repo().create(&new_flow).await {
            created.push(flow);
        }
    }

    // Process description-matched candidates (not caught by heuristic).
    for (source_id, target_id, txn_id, amount, date) in &extra_candidates {
        if existing_source_txns.contains(txn_id) {
            continue;
        }

        let new_flow = NewAccountFlow {
            portfolio_id,
            source_account_id: *source_id,
            target_account_id: *target_id,
            source_transaction_id: Some(*txn_id),
            target_transaction_id: None,
            amount: *amount,
            flow_date: *date,
            is_auto_detected: true,
        };

        if let Ok(flow) = state.flow_repo().create(&new_flow).await {
            created.push(flow);
        }
    }

    let total_detected = candidates.len() + extra_candidates.len();

    Ok(Json(serde_json::json!({
        "detected": total_detected,
        "created": created.len(),
        "flows": created
    })))
}

// ---------------------------------------------------------------------------
// Flow group handlers
// ---------------------------------------------------------------------------

/// Look up the portfolio_id that owns a flow group.
async fn flow_group_portfolio_id(state: &AppState, group_id: Uuid) -> Result<Uuid, AppError> {
    let row: (Uuid,) = sqlx::query_as("SELECT portfolio_id FROM flow_groups WHERE id = $1")
        .bind(group_id)
        .fetch_one(state.pool())
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => AppError::NotFound,
            _ => AppError::from(e),
        })?;
    Ok(row.0)
}

/// GET /api/flow-groups
pub async fn list_flow_groups(
    user: AuthUser,
    State(state): State<AppState>,
    Query(params): Query<PortfolioQuery>,
) -> Result<impl IntoResponse, AppError> {
    let portfolio_id = resolve_portfolio_id(&user, &state, params.portfolio_id).await?;

    let groups = state
        .flow_group_repo()
        .list_by_portfolio(portfolio_id)
        .await?;

    Ok(Json(groups))
}

/// POST /api/flow-groups
pub async fn create_flow_group(
    user: AuthUser,
    State(state): State<AppState>,
    Json(body): Json<CreateFlowGroupRequest>,
) -> Result<impl IntoResponse, AppError> {
    let portfolio_id = resolve_portfolio_id(&user, &state, body.portfolio_id).await?;

    let group = state
        .flow_group_repo()
        .create(portfolio_id, &body.name)
        .await?;

    Ok(Json(group))
}

/// PUT /api/flow-groups/:id
pub async fn update_flow_group(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateFlowGroupRequest>,
) -> Result<impl IntoResponse, AppError> {
    // Verify ownership: flow group -> portfolio -> user
    let portfolio_id = flow_group_portfolio_id(&state, id).await?;
    state
        .portfolio_repo()
        .verify_ownership(portfolio_id, user.user_id)
        .await?;

    let group = state.flow_group_repo().update(id, &body.name).await?;

    Ok(Json(group))
}

/// DELETE /api/flow-groups/:id
pub async fn delete_flow_group(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    // Verify ownership: flow group -> portfolio -> user
    let portfolio_id = flow_group_portfolio_id(&state, id).await?;
    state
        .portfolio_repo()
        .verify_ownership(portfolio_id, user.user_id)
        .await?;

    state.flow_group_repo().delete(id).await?;

    Ok(Json(serde_json::json!({"status": "deleted"})))
}
