//! Integration tests for Tier 2 semantic-categorization bootstrap +
//! persistence, and SONA flow-pattern confirm + resolve (issue #33).
//!
//! Against a real PostgreSQL test database (docker-compose.test.yml), this
//! file covers:
//!
//!  1. Seeding a portfolio with confirmed (category + subcategory present)
//!     transactions and running the Tier 2 bootstrap path
//!     (`finima_categorize::tier2::bootstrap_semantic`, the same library
//!     call `bootstrap_tier2` -- crates/finima-api/src/bin/bootstrap_tier2.rs
//!     -- makes), then persisting the resulting examples to the
//!     `embedding_index` table via `finima_db::repos::EmbeddingIndexRepo`
//!     and asserting the row count matches what was seeded.
//!
//!     NOTE: `bootstrap_tier2` (the bin) currently only builds the
//!     in-memory Jaccard/RuVector store and a `portfolios.sona_state`
//!     marker -- per its own doc comment, writing to `embedding_index` is
//!     "Phase 2" work no production code path performs yet. This test
//!     therefore exercises the real `EmbeddingIndexRepo` (the persistence
//!     layer that table exists for) directly against the same bootstrapped
//!     examples, to validate the row-count contract ahead of that wiring.
//!
//!  2. Calling the vector-aware categorization endpoint contract
//!     (`POST /api/categorize/with-vector`) with a fake, deterministic
//!     vector -- no live embedder/LLM required -- and asserting the
//!     seeded category/subcategory comes back via the Jaccard backend.
//!     A `sona`-gated companion test additionally seeds a RuVector store
//!     and proves the real `finima_categorize::tier2::ruvector_store::
//!     categorize_with_vector` vector-search path is what answers the
//!     request, not the Jaccard fallback (see test 2b below).
//!
//!  3. Confirming a flow via the flow-confirm endpoint contract
//!     (`PUT /api/flows/:id`), persisting a `flow_patterns` row, updating
//!     an in-process `RuVectorPatternMatcher` and its derived
//!     `flow_pattern_index_size` gauge stand-in (see (ii) below), then
//!     rebuilding a fresh `RuVectorPatternMatcher` from the persisted
//!     vector and calling
//!     `finima_analysis::flows::resolve_one_sided_flows_with_vectors` to
//!     confirm the stored target account comes back correctly.
//!     `sona`-feature-gated, mirroring the gate this function's own unit
//!     tests already use in `finima-analysis/src/flows.rs`.
//!
//! ## What this file honestly does and doesn't cover
//!
//! `finima-api` is a binary crate (no `lib.rs`), so its real Axum handlers
//! (`handlers::categorization::categorize_transaction_with_vector`,
//! `handlers::flows::update_flow`) and `AppState` are not constructible
//! from this integration-test crate -- see `tests/common/mod.rs`'s own doc
//! comment, and `auth_test.rs` / `authorization_test.rs`, which hit the
//! exact same structural wall. This is a known, pre-existing gap shared by
//! every integration test in this crate, not something specific to this
//! file, and it is NOT closed here. A `finima-api` `lib.rs` extraction
//! (moving the router/handlers/`AppState` into a library target with a
//! thin `main.rs` binary shim) would close it for every integration test
//! in this crate at once and is recommended as separate, dedicated
//! follow-up work -- it is out of scope for this file and has not been
//! attempted here.
//!
//! Given that constraint, this file draws an explicit line between two
//! different kinds of "real":
//!
//!  (i) **Library-level logic exercised via real library calls -- genuine,
//!      meaningful coverage:** `bootstrap_semantic`, `EmbeddingIndexRepo`,
//!      `FlowPatternRepo::upsert_confirmed`,
//!      `resolve_one_sided_flows_with_vectors`, and (as of this revision)
//!      the vector-path dispatch logic in
//!      `finima_categorize::tier2::ruvector_store::categorize_with_vector`
//!      and the `flow_pattern_index_size` gauge-resolution logic
//!      (mirroring `handlers::flows::resolve_flow_pattern_index_size`) are
//!      all called directly against real, unmocked implementations from
//!      the `finima-categorize` / `finima-analysis` / `finima-db` library
//!      crates. A regression in any of that logic will fail these tests.
//!
//!  (ii) **HTTP-handler-level glue -- NOT exercised, known gap:** the exact
//!      sequence of operations *inside* the real
//!      `handlers::flows::update_flow` and
//!      `handlers::categorization::categorize_transaction_with_vector`
//!      functions (routing, extractor wiring, the precise order handler
//!      code calls the library functions in, error-status mapping, and
//!      metrics recording against the real `finima_api::metrics::
//!      MetricsRegistry`, which this crate cannot even import) is
//!      reimplemented by hand below (`confirm_flow_handler`,
//!      `categorize_with_vector_handler`) rather than called. Where this
//!      revision duplicates a fix made to the real handler (the
//!      `flow_pattern_index_size` gauge-resolution logic, and the
//!      vector-vs-Jaccard dispatch), the test-local copy is kept in sync
//!      by hand and can drift from the real handler without either
//!      failing -- that is inherent to hand-duplicated glue and is exactly
//!      the gap a `lib.rs` extraction would close.
//!
//! No `#[ignore]` gate: mirrors `auth_test.rs` / `authorization_test.rs`,
//! which run unconditionally against the required test database rather
//! than being skipped by default.
//!
//! Run with:
//!   docker compose -f docker-compose.test.yml up -d
//!   TEST_DATABASE_URL=postgres://finima:test@localhost:5433/finima_test \
//!     APP_ENV=test cargo test -p finima-api --test tier2_flow_persistence_test
//!
//!   # Additionally, for the SONA flow-matcher + RuVector-categorization tests:
//!   TEST_DATABASE_URL=postgres://finima:test@localhost:5433/finima_test \
//!     APP_ENV=test cargo test -p finima-api --features sona \
//!     --test tier2_flow_persistence_test

mod common;

use std::sync::{Arc, RwLock};

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{Request, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{post, put};
use axum::{Json, Router};
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::Row;
use uuid::Uuid;

use finima_auth::middleware::{AuthUser, JwtSecret};
use finima_categorize::tier2::{
    bootstrap_semantic, EmbeddingStore, LabeledExample, SemanticCategorizer,
};
use finima_categorize::UncategorizedTransaction;
use finima_core::traits::{PortfolioRepo, UserRepo};

#[cfg(feature = "sona")]
use finima_analysis::sona::{
    FlowPattern, FlowPatternMatcher, RuVectorPatternMatcher, RuVectorPatternMatcherConfig,
};
#[cfg(feature = "sona")]
use finima_categorize::config::Tier2Config;
#[cfg(feature = "sona")]
use finima_categorize::tier2::ruvector_store::categorize_with_vector as ruvector_categorize_with_vector;
#[cfg(feature = "sona")]
use finima_categorize::tier2::{RuVectorEmbeddingStore, SemanticVectorIngest};

use common::{access_token_for, setup_test_db, TestAppState, TestClient};

// ---------------------------------------------------------------------------
// Deterministic fake embedding
// ---------------------------------------------------------------------------

/// Deterministic pseudo-random unit vector derived from `seed`, standing in
/// for a real embedder's output so these tests run without any external
/// LLM/embedder service. Same construction as the `unit_vec` helper already
/// used by `finima-categorize`'s and `finima-analysis`'s own `sona`/
/// `ruvector_store` unit tests (splitmix64 fill + L2 normalize).
fn fake_vector(seed: &str, dim: usize) -> Vec<f32> {
    let mut v = Vec::with_capacity(dim);
    let mut s: u64 = seed.bytes().fold(1469598103934665603u64, |acc, b| {
        (acc ^ b as u64).wrapping_mul(1099511628211)
    });
    for _ in 0..dim {
        s = s
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        v.push(((s >> 33) as f32) / (u32::MAX as f32) - 0.5);
    }
    let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-9);
    for x in &mut v {
        *x /= norm;
    }
    v
}

// ---------------------------------------------------------------------------
// Test-local router: mirrors the request/response contracts of
// POST /api/categorize/with-vector and PUT /api/flows/:id
// ---------------------------------------------------------------------------

const FLOW_VECTOR_DIM: usize = 32;

#[derive(Clone)]
struct Tier2RouterState {
    app: TestAppState,
    tier2: Arc<RwLock<EmbeddingStore>>,
    /// Mirrors `AppState::semantic_tier2_ruvector()`: `Some` only when a
    /// test explicitly seeds a RuVector store via
    /// `build_tier2_router_with_ruvector`. `None` reproduces the
    /// production fallback path (no RuVector backend configured / `sona`
    /// off), which `categorize_with_vector_handler` also handles below.
    #[cfg(feature = "sona")]
    tier2_ruvector: Option<Arc<RwLock<RuVectorEmbeddingStore>>>,
    /// Mirrors `AppState::flow_matcher_ruvector()`. Always present (as
    /// `Some` would be in production) so `confirm_flow_handler` can
    /// exercise the same "prefer the RuVector matcher's real count"
    /// resolution logic as `handlers::flows::resolve_flow_pattern_index_size`.
    #[cfg(feature = "sona")]
    flow_matcher_ruvector: Arc<RwLock<RuVectorPatternMatcher>>,
}

#[derive(Debug, Deserialize)]
struct CategorizeWithVectorRequest {
    transaction_id: Uuid,
    description: String,
    amount: Decimal,
    date: NaiveDate,
    #[serde(default)]
    mcc: Option<u16>,
    /// Mirrors the production request shape. Under the `sona` feature,
    /// when `Tier2RouterState::tier2_ruvector` is seeded (see
    /// `build_tier2_router_with_ruvector`), this vector drives the real
    /// `finima_categorize::tier2::ruvector_store::categorize_with_vector`
    /// query path -- see `categorize_with_vector_handler` below. Otherwise
    /// (no `sona` feature, or no RuVector store present) it is ignored and
    /// the Jaccard `EmbeddingStore::categorize` text-only fallback is used,
    /// matching the real handler's own fallback behavior.
    ///
    /// `#[allow(dead_code)]` is scoped to non-`sona` builds only: on a
    /// `sona` build this field is genuinely read (see
    /// `categorize_with_vector_endpoint_uses_ruvector_path_when_seeded`).
    #[serde(default)]
    #[cfg_attr(not(feature = "sona"), allow(dead_code))]
    precomputed_vector: Option<Vec<f32>>,
}

#[derive(Debug, Serialize)]
struct CategorizeWithVectorResponse {
    matched: bool,
    category: Option<String>,
    subcategory: Option<String>,
    confidence: Option<f64>,
    source_tier: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FlowActionRequest {
    action: String,
}

/// Middleware injecting the test JWT secret, so `AuthUser` can decode
/// tokens minted by `common::access_token_for`. Mirrors
/// `common::inject_jwt_secret` but doesn't need `TestAppState` as its
/// `State`, since this router's state is the combined `Tier2RouterState`.
async fn inject_test_jwt_secret(mut request: Request<Body>, next: Next) -> Response {
    request
        .extensions_mut()
        .insert(JwtSecret(common::TEST_JWT_SECRET.to_string()));
    next.run(request).await
}

/// Test-local mirror of
/// `categorization::categorize_transaction_with_vector`
/// (`finima-api/src/handlers/categorization.rs`). Under the `sona`
/// feature, when the caller supplies a vector AND
/// `Tier2RouterState::tier2_ruvector` is seeded, this dispatches to the
/// real `finima_categorize::tier2::ruvector_store::categorize_with_vector`
/// vector-search path -- matching the real handler's own
/// `#[cfg(feature = "sona")]` branch. Otherwise it falls back to the
/// Jaccard `EmbeddingStore::categorize` text-only path, also matching the
/// real handler. See module doc comment for why this is a hand-written
/// mirror rather than a call into the real handler.
async fn categorize_with_vector_handler(
    _user: AuthUser,
    State(rstate): State<Tier2RouterState>,
    Json(body): Json<CategorizeWithVectorRequest>,
) -> impl IntoResponse {
    let txn = UncategorizedTransaction {
        id: body.transaction_id,
        description: body.description,
        amount: body.amount,
        date: body.date,
        mcc: body.mcc,
    };

    let assignment = {
        #[cfg(feature = "sona")]
        {
            match (
                body.precomputed_vector.as_deref(),
                rstate.tier2_ruvector.as_ref(),
            ) {
                (Some(v), Some(store)) => {
                    let guard = store.read().expect("tier2_ruvector lock poisoned");
                    ruvector_categorize_with_vector(&guard, &txn, v)
                }
                _ => {
                    let guard = rstate.tier2.read().expect("tier2 store lock poisoned");
                    guard.categorize(&txn)
                }
            }
        }
        #[cfg(not(feature = "sona"))]
        {
            let guard = rstate.tier2.read().expect("tier2 store lock poisoned");
            guard.categorize(&txn)
        }
    };

    let response = match assignment {
        Some(a) => CategorizeWithVectorResponse {
            matched: true,
            category: Some(a.category),
            subcategory: Some(a.subcategory),
            confidence: Some(a.confidence),
            source_tier: Some(format!("{:?}", a.source_tier)),
        },
        None => CategorizeWithVectorResponse {
            matched: false,
            category: None,
            subcategory: None,
            confidence: None,
            source_tier: None,
        },
    };

    (StatusCode::OK, Json(response))
}

/// Test-local mirror of `flows::update_flow`'s "confirm" branch
/// (`finima-api/src/handlers/flows.rs`): verifies ownership, confirms the
/// flow, then upserts a `flow_patterns` row. Uses the deterministic
/// `fake_vector` in place of `AppState::embedder()`. Under `sona`, also
/// updates the in-process RuVector flow-pattern matcher and the
/// `flow_pattern_index_size` gauge stand-in (`TestAppState::
/// set_flow_pattern_index_size`), duplicating the resolution logic in
/// `handlers::flows::resolve_flow_pattern_index_size` (prefer the RuVector
/// matcher's real count; fall back to 0 otherwise).
async fn confirm_flow_handler(
    user: AuthUser,
    State(rstate): State<Tier2RouterState>,
    Path(id): Path<Uuid>,
    Json(body): Json<FlowActionRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    let app = &rstate.app;

    let flow = app
        .flow_repo()
        .find_by_id(id)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    app.portfolio_repo()
        .verify_ownership(flow.portfolio_id, user.user_id)
        .await
        .map_err(|_| StatusCode::FORBIDDEN)?;

    if body.action != "confirm" {
        return Err(StatusCode::BAD_REQUEST);
    }

    app.flow_repo()
        .confirm(id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let source_txn_id = flow
        .source_transaction_id
        .ok_or(StatusCode::UNPROCESSABLE_ENTITY)?;
    let source_txn = app
        .transaction_repo()
        .find_by_id(source_txn_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let vector = fake_vector(&source_txn.description, FLOW_VECTOR_DIM);
    let embedding_bytes: Vec<u8> = vector.iter().flat_map(|f| f.to_le_bytes()).collect();

    app.flow_pattern_repo()
        .upsert_confirmed(finima_db::repos::NewFlowPattern {
            portfolio_id: flow.portfolio_id,
            description_text: source_txn.description.clone(),
            source_account_id: flow.source_account_id,
            target_account_id: flow.target_account_id,
            confidence: 1.0,
            embedding: Some(embedding_bytes),
            embedding_dim: Some(FLOW_VECTOR_DIM as i32),
        })
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // In-memory matcher update + gauge resolution, mirroring
    // `handlers::flows::update_flow`'s confirm branch and
    // `resolve_flow_pattern_index_size`.
    #[cfg(feature = "sona")]
    {
        {
            let mut matcher = rstate
                .flow_matcher_ruvector
                .write()
                .expect("flow_matcher_ruvector lock poisoned");
            matcher.store_pattern_with_vector(
                FlowPattern {
                    description: source_txn.description.clone(),
                    source_account_id: flow.source_account_id,
                    target_account_id: flow.target_account_id,
                    confidence: 1.0,
                    match_count: 1,
                },
                &vector,
            );
            // Drop the write guard before reading `pattern_count()` below
            // -- std `RwLock` isn't reentrant, matching the real handler's
            // own `drop(matcher)` before calling
            // `resolve_flow_pattern_index_size`.
        }
        let count = rstate
            .flow_matcher_ruvector
            .read()
            .expect("flow_matcher_ruvector lock poisoned")
            .pattern_count() as i64;
        app.set_flow_pattern_index_size(count);
    }

    Ok((StatusCode::OK, Json(json!({"status": "confirmed"}))))
}

fn build_tier2_router(state: TestAppState, tier2: Arc<RwLock<EmbeddingStore>>) -> Router {
    #[cfg(feature = "sona")]
    {
        build_tier2_router_with_ruvector(state, tier2, None)
    }
    #[cfg(not(feature = "sona"))]
    {
        let rstate = Tier2RouterState { app: state, tier2 };
        Router::new()
            .route(
                "/api/categorize/with-vector",
                post(categorize_with_vector_handler),
            )
            .route("/api/flows/{id}", put(confirm_flow_handler))
            .layer(middleware::from_fn(inject_test_jwt_secret))
            .with_state(rstate)
    }
}

/// Like `build_tier2_router`, but additionally accepts a RuVector Tier 2
/// store (`tier2_ruvector`), mirroring `AppState::semantic_tier2_ruvector()`
/// being `Some`. Always builds a fresh `RuVectorPatternMatcher` for the
/// flow-pattern side, mirroring a successful RuVector construction at
/// startup (see `AppState::new`).
#[cfg(feature = "sona")]
fn build_tier2_router_with_ruvector(
    state: TestAppState,
    tier2: Arc<RwLock<EmbeddingStore>>,
    tier2_ruvector: Option<Arc<RwLock<RuVectorEmbeddingStore>>>,
) -> Router {
    let flow_matcher_ruvector = Arc::new(RwLock::new(
        RuVectorPatternMatcher::new(RuVectorPatternMatcherConfig {
            dim: FLOW_VECTOR_DIM,
            ..RuVectorPatternMatcherConfig::default()
        })
        .expect("build RuVectorPatternMatcher for test router"),
    ));
    let rstate = Tier2RouterState {
        app: state,
        tier2,
        tier2_ruvector,
        flow_matcher_ruvector,
    };
    Router::new()
        .route(
            "/api/categorize/with-vector",
            post(categorize_with_vector_handler),
        )
        .route("/api/flows/{id}", put(confirm_flow_handler))
        .layer(middleware::from_fn(inject_test_jwt_secret))
        .with_state(rstate)
}

// ---------------------------------------------------------------------------
// Small SQL seeding helpers
// ---------------------------------------------------------------------------

async fn seed_account(pool: &sqlx::PgPool, portfolio_id: Uuid, name: &str) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO accounts (id, portfolio_id, name, account_type, opening_balance, is_primary_income, created_at)
         VALUES ($1, $2, $3, 'checking', 0, false, NOW())",
    )
    .bind(id)
    .bind(portfolio_id)
    .bind(name)
    .execute(pool)
    .await
    .expect("seed account");
    id
}

// ---------------------------------------------------------------------------
// 1. Tier 2 bootstrap -> embedding_index persistence
// ---------------------------------------------------------------------------

#[tokio::test]
async fn bootstrap_semantic_persists_embedding_index_rows_matching_seed_count() {
    let pool = setup_test_db().await;
    let state = TestAppState::new(pool.clone());

    // Fresh, isolated user/portfolio/account so this test's row counts
    // aren't affected by other tests running concurrently against the same
    // database (cargo test runs test fns in parallel by default).
    let email = format!("tier2-bootstrap-{}@finima.local", Uuid::new_v4());
    let user = state
        .user_repo()
        .create_user(&email, "Tier2 Bootstrap")
        .await
        .expect("create user");
    let portfolio = state
        .portfolio_repo()
        .create(user.id, "Tier2 Bootstrap Portfolio")
        .await
        .expect("create portfolio");
    let account_id = seed_account(&pool, portfolio.id, "Checking").await;

    // Confirmed (category + subcategory present) transactions -- the exact
    // corpus `bootstrap_tier2` queries for. The issue suggests seeding 50;
    // we use a smaller, deterministic N here since 50 hand-written rows add
    // no additional coverage over this set -- the bootstrap/report/count
    // logic being verified is count-based, not scale-sensitive.
    let seed_examples: Vec<(&str, &str, &str)> = vec![
        ("STARBUCKS COFFEE #1201", "food_dining", "coffee_shops"),
        ("STARBUCKS COFFEE #1450", "food_dining", "coffee_shops"),
        ("SHELL OIL 44210", "transportation", "gas_fuel"),
        ("SHELL OIL 51120", "transportation", "gas_fuel"),
        ("WHOLEFDS MKT #10234", "food_dining", "groceries"),
        ("TRADER JOES #287", "food_dining", "groceries"),
        ("NETFLIX.COM", "entertainment", "streaming_services"),
        ("SPOTIFY USA", "entertainment", "streaming_services"),
        ("COMCAST CABLE", "utilities", "internet_cable"),
        ("ELECTRIC CO PAYMENT", "utilities", "electricity"),
        ("AMZN MKTP US*RT4K2", "shopping", "online_retail"),
        ("TARGET T-2847", "shopping", "general_merchandise"),
    ];
    for (i, (desc, cat, sub)) in seed_examples.iter().enumerate() {
        sqlx::query(
            "INSERT INTO transactions
                (id, account_id, date, amount, description, original_description,
                 category, subcategory, llm_confidence, dedup_hash, created_at)
             VALUES ($1, $2, $3, $4, $5, $5, $6, $7, 0.93, $8, NOW())",
        )
        .bind(Uuid::new_v4())
        .bind(account_id)
        .bind(NaiveDate::from_ymd_opt(2026, 1, 1 + i as u32).unwrap())
        .bind(Decimal::new(-1000 - i as i64, 2))
        .bind(*desc)
        .bind(*cat)
        .bind(*sub)
        .bind(format!("tier2-bootstrap-{i}-{account_id}"))
        .execute(&pool)
        .await
        .expect("seed labeled transaction");
    }

    // --- Run the Tier 2 bootstrap path -------------------------------
    // Same query shape (category + subcategory both present), same
    // `LabeledExample` mapping, and the same `bootstrap_semantic` call as
    // `crates/finima-api/src/bin/bootstrap_tier2.rs`'s `main()`. That
    // `main()` isn't factored into a callable library fn -- it's inline in
    // `#[tokio::main] async fn main()` -- so this test calls the library
    // functions it delegates to directly rather than shelling out to
    // `cargo run --bin bootstrap_tier2`, which would need a subprocess
    // plus the bin's full YAML config-loading just to reach the same code.
    let rows = sqlx::query(
        "SELECT t.description, t.category, t.subcategory, t.llm_confidence
           FROM transactions t
           JOIN accounts a ON a.id = t.account_id
          WHERE a.portfolio_id = $1
            AND t.category IS NOT NULL
            AND t.subcategory IS NOT NULL",
    )
    .bind(portfolio.id)
    .fetch_all(&pool)
    .await
    .expect("query labeled transactions");

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
            // Phase 1 bootstrap is bring-your-own-vectors; no embedder is
            // configured here, matching the bin's own default behavior.
            vector: None,
        })
        .collect();

    let mut store = EmbeddingStore::new(0.65);
    let (report, err) = bootstrap_semantic(&mut store, examples.clone(), 0);

    assert!(err.is_none(), "bootstrap should not report an error");
    assert_eq!(report.offered, seed_examples.len());
    assert_eq!(report.inserted, seed_examples.len());
    assert_eq!(report.rejected, 0);
    assert_eq!(store.len(), seed_examples.len());

    // --- Persist to the `embedding_index` Postgres table -------------
    // See module + file-header doc comments: `bootstrap_tier2` itself does
    // not yet write here. `EmbeddingIndexRepo` is the real repo this table
    // exists for; exercise it directly against the bootstrapped examples.
    for example in &examples {
        state
            .embedding_index_repo()
            .insert(finima_db::repos::NewEmbeddingIndex {
                portfolio_id: portfolio.id,
                description: example.description.clone(),
                description_normalized: example.description.to_lowercase(),
                embedding: None,
                embedding_dim: None,
                category: example.category.clone(),
                subcategory: example.subcategory.clone(),
                confidence: example.confidence,
                source_tier: "semantic_search".to_string(),
            })
            .await
            .expect("insert embedding_index row");
    }

    let count = state
        .embedding_index_repo()
        .count_for_portfolio(portfolio.id)
        .await
        .expect("count embedding_index rows");
    assert_eq!(
        count,
        seed_examples.len() as i64,
        "embedding_index row count should match the seeded, bootstrapped example count"
    );
}

// ---------------------------------------------------------------------------
// 2. POST /api/categorize/with-vector
// ---------------------------------------------------------------------------

#[tokio::test]
async fn categorize_with_vector_endpoint_matches_seeded_category() {
    let pool = setup_test_db().await;
    let state = TestAppState::new(pool.clone());
    let email = format!("tier2-categorize-{}@finima.local", Uuid::new_v4());
    let user = state
        .user_repo()
        .create_user(&email, "Tier2 Categorize")
        .await
        .expect("create user");

    let mut store = EmbeddingStore::new(0.5);
    store.insert(
        "STARBUCKS COFFEE #1234",
        "food_dining",
        "coffee_shops",
        0.95,
    );
    store.insert("SHELL OIL 44210", "transportation", "gas_fuel", 0.90);
    let tier2_store = Arc::new(RwLock::new(store));

    let token = access_token_for(user.id, &user.email);
    let router = build_tier2_router(state.clone(), tier2_store.clone());
    let client = TestClient::new(router).with_auth(&token);

    let body = json!({
        "transaction_id": Uuid::new_v4(),
        "description": "STARBUCKS COFFEE #5678",
        "amount": -6.25,
        "date": "2026-01-15",
        "mcc": null,
        // Deterministic fake vector standing in for a live embedder. The
        // Jaccard backend under test ignores it entirely -- proving this
        // endpoint's contract works without any external LLM/embedder
        // service, per the issue's acceptance criteria.
        "precomputed_vector": fake_vector("STARBUCKS COFFEE #5678", 8),
    });

    let (status, resp) = client.post("/api/categorize/with-vector", &body).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(resp["matched"], true);
    assert_eq!(resp["category"], "food_dining");
    assert_eq!(resp["subcategory"], "coffee_shops");
    assert_eq!(resp["source_tier"], "SemanticSearch");

    // Unauthenticated requests are rejected -- mirrors production's
    // `AuthUser` extractor requirement on this route.
    let anon_router = build_tier2_router(state, tier2_store);
    let anon_client = TestClient::new(anon_router);
    let (status, _) = anon_client.post("/api/categorize/with-vector", &body).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// ---------------------------------------------------------------------------
// 2b. POST /api/categorize/with-vector -- proves the RuVector vector-search
//     path is actually exercised, not just that a category comes back
//     (sona-gated; see module doc comment (i)/(ii)).
// ---------------------------------------------------------------------------

/// Seeds a Jaccard `EmbeddingStore` and a RuVector `RuVectorEmbeddingStore`
/// with the SAME description text but DIFFERENT (mutually exclusive)
/// category/subcategory labels, then asserts the response matches the
/// RuVector label. If `categorize_with_vector_handler` ever regressed to
/// unconditionally calling `EmbeddingStore::categorize` (as it did before
/// this fix, and as the pre-existing Test 2 above cannot detect, since it
/// never seeds a RuVector store at all), this test would get the WRONG
/// (Jaccard) answer and fail -- it cannot pass "by accident" the way a test
/// that only checks `matched == true` could.
#[cfg(feature = "sona")]
#[tokio::test]
async fn categorize_with_vector_endpoint_uses_ruvector_path_when_seeded() {
    let pool = setup_test_db().await;
    let state = TestAppState::new(pool.clone());
    let email = format!("tier2-ruvector-{}@finima.local", Uuid::new_v4());
    let user = state
        .user_repo()
        .create_user(&email, "Tier2 RuVector")
        .await
        .expect("create user");

    let description = "AUTOPAY UTILITY BILL XYZ";
    let dim = 32;
    let vector = fake_vector(description, dim);

    // Jaccard store: exact-text entry tagged with a WRONG category. Jaccard
    // n-gram similarity against an identical (normalized) description is
    // 1.0, so if the handler fell through to this store despite a vector +
    // RuVector store both being available, it would confidently return
    // this WRONG label.
    let mut jaccard = EmbeddingStore::new(0.5);
    jaccard.insert(description, "entertainment", "streaming_services", 0.99);
    let tier2_store = Arc::new(RwLock::new(jaccard));

    // RuVector store: the SAME description's vector tagged with the
    // CORRECT category. Only reachable via `categorize_with_vector`.
    let mut ruvector = RuVectorEmbeddingStore::new(
        Tier2Config {
            dim,
            ..Tier2Config::default()
        },
        0.0,
    )
    .expect("build RuVectorEmbeddingStore");
    assert!(
        ruvector.learn_with_vector(description, "utilities", "bill_pay", 0.95, Some(&vector)),
        "seeding the RuVector store should succeed"
    );
    let ruvector_store = Arc::new(RwLock::new(ruvector));

    let token = access_token_for(user.id, &user.email);
    let router = build_tier2_router_with_ruvector(
        state.clone(),
        tier2_store.clone(),
        Some(ruvector_store.clone()),
    );
    let client = TestClient::new(router).with_auth(&token);

    let body = json!({
        "transaction_id": Uuid::new_v4(),
        "description": description,
        "amount": -120.00,
        "date": "2026-01-15",
        "mcc": null,
        "precomputed_vector": vector,
    });

    let (status, resp) = client.post("/api/categorize/with-vector", &body).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(resp["matched"], true);
    // The load-bearing assertions: these prove the RuVector path answered
    // the request. Getting "entertainment"/"streaming_services" here would
    // mean the Jaccard fallback ran instead -- see the sabotage-test
    // verification in the fix's report for empirical proof this actually
    // fails when the RuVector dispatch is broken.
    assert_eq!(resp["category"], "utilities");
    assert_eq!(resp["subcategory"], "bill_pay");
}

// ---------------------------------------------------------------------------
// 3. PUT /api/flows/:id -> flow_patterns persistence -> resolve via vector
//    matcher (sona-gated, matching the gate on
//    `resolve_one_sided_flows_with_vectors` itself)
// ---------------------------------------------------------------------------

#[cfg(feature = "sona")]
#[tokio::test]
async fn confirm_flow_pattern_resolves_via_vector_matcher() {
    // `FlowPattern` / `RuVectorPatternMatcher` / `RuVectorPatternMatcherConfig`
    // come from the module-level `#[cfg(feature = "sona")]` imports.
    use finima_analysis::FlowCandidate;

    let pool = setup_test_db().await;
    let state = TestAppState::new(pool.clone());
    let email = format!("tier2-flow-{}@finima.local", Uuid::new_v4());
    let user = state
        .user_repo()
        .create_user(&email, "Tier2 Flow")
        .await
        .expect("create user");
    let portfolio = state
        .portfolio_repo()
        .create(user.id, "Tier2 Flow Portfolio")
        .await
        .expect("create portfolio");

    let source_account_id = seed_account(&pool, portfolio.id, "Checking").await;
    let target_account_id = seed_account(&pool, portfolio.id, "Savings").await;

    let description = "AUTOPAY AMEX CARD";
    let source_txn_id = Uuid::new_v4();
    let target_txn_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO transactions
            (id, account_id, date, amount, description, original_description, dedup_hash, created_at)
         VALUES ($1, $2, '2026-01-10', -650.00, $3, $3, $4, NOW())",
    )
    .bind(source_txn_id)
    .bind(source_account_id)
    .bind(description)
    .bind(format!("tier2-flow-src-{source_txn_id}"))
    .execute(&pool)
    .await
    .expect("create source transaction");
    sqlx::query(
        "INSERT INTO transactions
            (id, account_id, date, amount, description, original_description, dedup_hash, created_at)
         VALUES ($1, $2, '2026-01-10', 650.00, 'PAYMENT RECEIVED', 'PAYMENT RECEIVED', $3, NOW())",
    )
    .bind(target_txn_id)
    .bind(target_account_id)
    .bind(format!("tier2-flow-tgt-{target_txn_id}"))
    .execute(&pool)
    .await
    .expect("create target transaction");

    let flow = state
        .flow_repo()
        .create(&finima_db::NewAccountFlow {
            portfolio_id: portfolio.id,
            source_account_id,
            target_account_id,
            source_transaction_id: Some(source_txn_id),
            target_transaction_id: Some(target_txn_id),
            amount: Decimal::new(65000, 2),
            flow_date: NaiveDate::from_ymd_opt(2026, 1, 10).unwrap(),
            is_auto_detected: true,
        })
        .await
        .expect("create account flow");

    // --- Confirm the flow via PUT /api/flows/:id ----------------------
    let token = access_token_for(user.id, &user.email);
    let router = build_tier2_router(
        state.clone(),
        Arc::new(RwLock::new(EmbeddingStore::new(0.5))),
    );
    let client = TestClient::new(router).with_auth(&token);
    let (status, resp) = client
        .put(
            &format!("/api/flows/{}", flow.id),
            &json!({"action": "confirm"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(resp["status"], "confirmed");

    // --- The confirm updated the flow_pattern_index_size gauge stand-in -
    // Exercises the same "prefer the RuVector matcher's real count" logic
    // as `handlers::flows::resolve_flow_pattern_index_size` (duplicated in
    // `confirm_flow_handler` above, since the real private helper lives in
    // the `finima-api` binary crate and isn't callable from here). One
    // pattern was stored by the confirm above, so the count must be 1 --
    // not 0 (the stub/no-op value) and not left over from a previous test,
    // since each test builds its own fresh router/matcher.
    assert_eq!(
        state.flow_pattern_index_size(),
        1,
        "flow_pattern_index_size should reflect the one pattern stored by confirm"
    );

    // --- The confirm persisted a flow_patterns row ---------------------
    let patterns = state
        .flow_pattern_repo()
        .list_for_source(source_account_id)
        .await
        .expect("list flow patterns");
    let persisted = patterns
        .iter()
        .find(|p| p.description_text == description)
        .expect("confirmed pattern should be persisted");
    assert_eq!(persisted.target_account_id, target_account_id);
    assert_eq!(persisted.embedding_dim, Some(FLOW_VECTOR_DIM as i32));

    let persisted_bytes = persisted
        .embedding
        .as_ref()
        .expect("embedding bytes should be stored");
    let persisted_vector: Vec<f32> = persisted_bytes
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    assert_eq!(
        persisted_vector,
        fake_vector(description, FLOW_VECTOR_DIM),
        "persisted embedding should round-trip the same deterministic vector"
    );

    // --- Rebuild the vector-aware flow-pattern matcher and resolve ----
    // "Rebuild the relevant store": `AppState` itself can't be
    // reconstructed outside the finima-api binary crate (see module doc
    // comment), so this rebuilds the RuVector matcher that
    // `flows::update_flow`'s confirm branch feeds via
    // `state.flow_matcher_ruvector()` -- fed here from the vector we just
    // read back out of Postgres, proving the DB round-trip is load-bearing
    // rather than assumed.
    let mut matcher = RuVectorPatternMatcher::new(RuVectorPatternMatcherConfig {
        dim: FLOW_VECTOR_DIM,
        ..RuVectorPatternMatcherConfig::default()
    })
    .expect("build RuVectorPatternMatcher");
    matcher.store_pattern_with_vector(
        FlowPattern {
            description: description.to_string(),
            source_account_id,
            target_account_id,
            confidence: 1.0,
            match_count: 1,
        },
        &persisted_vector,
    );

    let mut candidates = vec![FlowCandidate {
        source_account_id,
        target_account_id: None,
        source_transaction_id: source_txn_id,
        target_transaction_id: None,
        amount: Decimal::new(65000, 2),
        flow_date: NaiveDate::from_ymd_opt(2026, 2, 10).unwrap(),
        is_transfer_like: true,
    }];
    let descriptions = vec![description.to_string()];
    let query_vectors = vec![persisted_vector.clone()];

    finima_analysis::flows::resolve_one_sided_flows_with_vectors(
        &mut candidates,
        &descriptions,
        &query_vectors,
        &matcher,
        0.0,
    );

    assert_eq!(
        candidates[0].target_account_id,
        Some(target_account_id),
        "resolve_one_sided_flows_with_vectors should recover the confirmed target account"
    );
}
