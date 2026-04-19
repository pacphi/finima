use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, RwLock};

use sqlx::PgPool;
use uuid::Uuid;

use finima_analysis::sona::{FlowPatternMatcher, StubPatternMatcher};
use finima_auth::EmailSender;
use finima_categorize::tier2::{EmbeddingStore, SemanticCategorizer};
use finima_categorize::{CascadeEngine, MerchantRegistry};

#[cfg(feature = "sona")]
use finima_analysis::sona::{RuVectorPatternMatcher, RuVectorPatternMatcherConfig};
#[cfg(feature = "sona")]
use finima_categorize::tier2::RuVectorEmbeddingStore;
use finima_db::repos::FlowPatternRepo;
use finima_db::{
    PgAccountRepo, PgBudgetRepo, PgFlowGroupRepo, PgFlowRepo, PgMagicLinkRepo, PgOverrideRepo,
    PgPortfolioRepo, PgRecurringRepo, PgSavingsGoalRepo, PgSessionRepo, PgTransactionRepo,
    PgUploadRepo, PgUserRepo,
};
use finima_feed::CachedFeedService;
use finima_llm::LlmClient;

use crate::config::AppConfig;
use crate::metrics::MetricsRegistry;
use crate::storage::ObjectStorage;
use crate::ws::WsConnectionManager;

use finima_embed::{EmbeddingProvider, NoopEmbedder};

/// Status of an on-demand categorization job.
#[derive(Clone, serde::Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum CategorizationJobStatus {
    Running,
    Complete {
        total: usize,
        flagged: usize,
        categories: Vec<crate::ws::CategoryCount>,
    },
    Failed {
        error: String,
    },
}

// LLM loading status constants.
const LLM_LOADING: u8 = 0;
const LLM_READY: u8 = 1;
const LLM_FAILED: u8 = 2;
const LLM_DISABLED: u8 = 3;

/// Shared application state available to all Axum handlers.
///
/// Wrapped in `Arc` internally so that cloning is cheap and all handlers
/// share the same pool, config, and email sender instance.
#[derive(Clone)]
pub struct AppState {
    inner: Arc<InnerState>,
}

struct InnerState {
    pub pool: PgPool,
    pub config: AppConfig,
    pub email_sender: Box<dyn EmailSender>,
    pub user_repo: PgUserRepo,
    pub portfolio_repo: PgPortfolioRepo,
    pub account_repo: PgAccountRepo,
    pub magic_link_repo: PgMagicLinkRepo,
    pub session_repo: PgSessionRepo,
    pub transaction_repo: PgTransactionRepo,
    pub upload_repo: PgUploadRepo,
    pub recurring_repo: PgRecurringRepo,
    pub override_repo: PgOverrideRepo,
    pub budget_repo: PgBudgetRepo,
    pub savings_goal_repo: PgSavingsGoalRepo,
    pub flow_repo: PgFlowRepo,
    pub flow_group_repo: PgFlowGroupRepo,
    pub flow_pattern_repo: Arc<FlowPatternRepo>,
    /// Shared flow-pattern matcher (ADR-017). Falls back to
    /// `StubPatternMatcher` when the `sona` feature is disabled or when
    /// RuVector construction fails.
    pub flow_matcher: Arc<RwLock<dyn FlowPatternMatcher>>,
    /// Concrete RuVector flow-pattern matcher handle used by callers
    /// that have precomputed embeddings.
    #[cfg(feature = "sona")]
    pub flow_matcher_ruvector: Option<Arc<RwLock<RuVectorPatternMatcher>>>,
    pub ws_manager: WsConnectionManager,
    /// LLM client loaded in the background. `None` until the model is ready
    /// (or if loading failed).
    pub llm_client: RwLock<Option<Arc<dyn LlmClient>>>,
    /// LLM loading status: 0 = loading, 1 = ready, 2 = failed.
    pub llm_status: AtomicU8,
    pub object_storage: ObjectStorage,
    pub feed_service: CachedFeedService,
    /// Tracks in-flight and completed on-demand categorization jobs.
    /// Key is (user_id, account_id).
    pub categorization_jobs: RwLock<HashMap<(Uuid, Uuid), CategorizationJobStatus>>,
    /// Tracks per-upload categorization progress (upload_id → (categorized, total)).
    /// Written by the categorization pipeline, read by the upload status endpoint.
    pub upload_categorization_progress: RwLock<HashMap<Uuid, (usize, usize)>>,
    /// In-memory merchant registry for the Tier 0 categorization cascade.
    /// Shared across requests so that LLM-learned merchants persist for the
    /// lifetime of the process.
    pub merchant_registry: Arc<RwLock<MerchantRegistry>>,
    /// Shared Tier 2 semantic categorizer (Jaccard by default; RuVector
    /// HNSW when `finima-categorize/sona` is enabled). Built once at
    /// startup from `categorize.tier2` + `categorize.semantic_min_confidence`.
    /// Falls back to an empty Jaccard `EmbeddingStore` if construction fails.
    pub semantic_tier2: Arc<RwLock<dyn SemanticCategorizer>>,
    /// Concrete RuVector store shared with the vector-aware Tier 2 handler
    /// (`POST /api/categorize/with-vector`). Populated only when the
    /// `sona` feature is enabled AND the YAML-resolved backend is RuVector;
    /// otherwise `None` and the vector handler falls back to the trait-level
    /// text-only probe.
    #[cfg(feature = "sona")]
    pub semantic_tier2_ruvector: Option<Arc<RwLock<RuVectorEmbeddingStore>>>,
    /// Shared embedding provider (ADR-017 Phase 3.C). Always present —
    /// falls back to `NoopEmbedder` when the configured backend is
    /// `none` or when the matching Cargo feature is disabled.
    pub embedder: Arc<dyn EmbeddingProvider>,
    /// Optional metrics registry. Set via `set_metrics` after construction
    /// so handlers can record Tier 2 / flow-pattern / bootstrap counters.
    pub metrics: RwLock<Option<MetricsRegistry>>,
    /// Set to `true` when the application is shutting down.
    /// Background tasks should check this and stop work promptly.
    pub shutdown: AtomicBool,
}

impl AppState {
    /// Creates application state without an LLM client so the server can start
    /// accepting requests immediately. Call `spawn_llm_loader` afterwards to
    /// load the LLM backend in the background.
    pub fn new(
        pool: PgPool,
        config: AppConfig,
        email_sender: Box<dyn EmailSender>,
        object_storage: ObjectStorage,
        feed_service: CachedFeedService,
    ) -> Self {
        let user_repo = PgUserRepo::new(pool.clone());
        let portfolio_repo = PgPortfolioRepo::new(pool.clone());
        let account_repo = PgAccountRepo::new(pool.clone());
        let magic_link_repo = PgMagicLinkRepo::new(pool.clone());
        let session_repo = PgSessionRepo::new(pool.clone());
        let transaction_repo = PgTransactionRepo::new(pool.clone());
        let upload_repo = PgUploadRepo::new(pool.clone());
        let recurring_repo = PgRecurringRepo::new(pool.clone());
        let override_repo = PgOverrideRepo::new(pool.clone());
        let budget_repo = PgBudgetRepo::new(pool.clone());
        let savings_goal_repo = PgSavingsGoalRepo::new(pool.clone());
        let flow_repo = PgFlowRepo::new(pool.clone());
        let flow_group_repo = PgFlowGroupRepo::new(pool.clone());
        let ws_manager = WsConnectionManager::new();

        // Build the merchant registry and load seed data so Tier 0
        // categorization works from the first request.
        let mut registry = MerchantRegistry::with_defaults();
        let seed_count = registry
            .load_seed_merchants(finima_categorize::SEED_MERCHANTS_JSON)
            .unwrap_or_else(|e| {
                tracing::warn!("Failed to load seed merchants: {}", e);
                0
            });
        tracing::info!(seed_count, "merchant registry initialized");
        let merchant_registry = Arc::new(RwLock::new(registry));

        // Build the Tier 2 semantic store from config, with a safe fallback
        // to an empty Jaccard store on error. Construction errors are only
        // possible for the RuVector backend; the Jaccard backend is
        // infallible.
        let categorize_cfg: finima_categorize::CategorizeConfig = config.categorize.clone().into();
        let semantic_tier2: Arc<RwLock<dyn SemanticCategorizer>> =
            match CascadeEngine::build_semantic_from_config(
                &categorize_cfg.tier2,
                categorize_cfg.semantic_min_confidence,
            ) {
                Ok(store) => store,
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        "Tier 2 semantic backend construction failed; falling back to empty Jaccard store"
                    );
                    Arc::new(RwLock::new(EmbeddingStore::new(
                        categorize_cfg.semantic_min_confidence,
                    )))
                }
            };

        // Build a typed RuVector handle only when the `sona` feature is
        // compiled in and the YAML-resolved backend actually asks for it.
        // This mirrors `semantic_tier2` construction so both views agree.
        #[cfg(feature = "sona")]
        let semantic_tier2_ruvector: Option<Arc<RwLock<RuVectorEmbeddingStore>>> = {
            use finima_categorize::config::Tier2Backend;
            if categorize_cfg.tier2.resolved_backend() == Tier2Backend::RuVector {
                match RuVectorEmbeddingStore::new(
                    categorize_cfg.tier2.clone(),
                    categorize_cfg.semantic_min_confidence,
                ) {
                    Ok(s) => Some(Arc::new(RwLock::new(s))),
                    Err(e) => {
                        tracing::warn!(error = %e, "RuVector Tier 2 handle construction failed");
                        None
                    }
                }
            } else {
                None
            }
        };

        // Build the flow-pattern repo (Phase 2.A) and matcher (Phase 2.B).
        let flow_pattern_repo = Arc::new(FlowPatternRepo::new(pool.clone()));

        #[cfg(feature = "sona")]
        let (flow_matcher, flow_matcher_ruvector): (
            Arc<RwLock<dyn FlowPatternMatcher>>,
            Option<Arc<RwLock<RuVectorPatternMatcher>>>,
        ) = {
            let cfg = RuVectorPatternMatcherConfig {
                dim: categorize_cfg.tier2.dim,
                hnsw_m: categorize_cfg.tier2.hnsw_m,
                hnsw_ef_construction: categorize_cfg.tier2.hnsw_ef_construction,
                hnsw_ef_search: categorize_cfg.tier2.hnsw_ef_search,
            };
            match RuVectorPatternMatcher::new(cfg) {
                Ok(m) => {
                    let arc = Arc::new(RwLock::new(m));
                    // The concrete Arc isn't coerceable to the trait-object
                    // Arc directly; build a second instance for the trait
                    // handle so both views are independent (acceptable since
                    // the trait-level methods are no-ops on this backend).
                    let stub: Arc<RwLock<dyn FlowPatternMatcher>> =
                        Arc::new(RwLock::new(StubPatternMatcher));
                    (stub, Some(arc))
                }
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        "RuVector flow-pattern matcher construction failed; falling back to StubPatternMatcher"
                    );
                    let stub: Arc<RwLock<dyn FlowPatternMatcher>> =
                        Arc::new(RwLock::new(StubPatternMatcher));
                    (stub, None)
                }
            }
        };

        #[cfg(not(feature = "sona"))]
        let flow_matcher: Arc<RwLock<dyn FlowPatternMatcher>> =
            Arc::new(RwLock::new(StubPatternMatcher));

        // Build the embedding provider from config. NoopEmbedder is the
        // safe fallback; heavy backends are gated behind Cargo features
        // so the default build stays lean.
        let embedder: Arc<dyn EmbeddingProvider> = build_embedder(&config);

        Self {
            inner: Arc::new(InnerState {
                pool,
                config,
                email_sender,
                user_repo,
                portfolio_repo,
                account_repo,
                magic_link_repo,
                session_repo,
                transaction_repo,
                upload_repo,
                recurring_repo,
                override_repo,
                budget_repo,
                savings_goal_repo,
                flow_repo,
                flow_group_repo,
                flow_pattern_repo,
                flow_matcher,
                #[cfg(feature = "sona")]
                flow_matcher_ruvector,
                ws_manager,
                llm_client: RwLock::new(None),
                llm_status: AtomicU8::new(LLM_LOADING),
                object_storage,
                feed_service,
                categorization_jobs: RwLock::new(HashMap::new()),
                upload_categorization_progress: RwLock::new(HashMap::new()),
                merchant_registry,
                semantic_tier2,
                #[cfg(feature = "sona")]
                semantic_tier2_ruvector,
                embedder,
                metrics: RwLock::new(None),
                shutdown: AtomicBool::new(false),
            }),
        }
    }

    /// Set the LLM client after successful background loading.
    pub fn set_llm_client(&self, client: Arc<dyn LlmClient>) {
        *self
            .inner
            .llm_client
            .write()
            .expect("llm_client lock poisoned") = Some(client);
        self.inner.llm_status.store(LLM_READY, Ordering::Release);
    }

    /// Mark the LLM backend as failed (init error, feature not enabled, etc.).
    pub fn set_llm_failed(&self) {
        self.inner.llm_status.store(LLM_FAILED, Ordering::Release);
    }

    /// Mark the LLM as intentionally disabled (provider = "none").
    pub fn set_llm_disabled(&self) {
        self.inner.llm_status.store(LLM_DISABLED, Ordering::Release);
    }

    /// LLM loading status: `"loading"`, `"ready"`, `"failed"`, or `"disabled"`.
    pub fn llm_status(&self) -> &'static str {
        match self.inner.llm_status.load(Ordering::Acquire) {
            LLM_READY => "ready",
            LLM_FAILED => "failed",
            LLM_DISABLED => "disabled",
            _ => "loading",
        }
    }

    pub fn pool(&self) -> &PgPool {
        &self.inner.pool
    }

    pub fn config(&self) -> &AppConfig {
        &self.inner.config
    }

    pub fn email_sender(&self) -> &dyn EmailSender {
        self.inner.email_sender.as_ref()
    }

    pub fn user_repo(&self) -> &PgUserRepo {
        &self.inner.user_repo
    }

    pub fn portfolio_repo(&self) -> &PgPortfolioRepo {
        &self.inner.portfolio_repo
    }

    pub fn account_repo(&self) -> &PgAccountRepo {
        &self.inner.account_repo
    }

    pub fn magic_link_repo(&self) -> &PgMagicLinkRepo {
        &self.inner.magic_link_repo
    }

    pub fn session_repo(&self) -> &PgSessionRepo {
        &self.inner.session_repo
    }

    pub fn transaction_repo(&self) -> &PgTransactionRepo {
        &self.inner.transaction_repo
    }

    pub fn upload_repo(&self) -> &PgUploadRepo {
        &self.inner.upload_repo
    }

    pub fn recurring_repo(&self) -> &PgRecurringRepo {
        &self.inner.recurring_repo
    }

    pub fn override_repo(&self) -> &PgOverrideRepo {
        &self.inner.override_repo
    }

    pub fn budget_repo(&self) -> &PgBudgetRepo {
        &self.inner.budget_repo
    }

    pub fn savings_goal_repo(&self) -> &PgSavingsGoalRepo {
        &self.inner.savings_goal_repo
    }

    pub fn flow_repo(&self) -> &PgFlowRepo {
        &self.inner.flow_repo
    }

    pub fn flow_group_repo(&self) -> &PgFlowGroupRepo {
        &self.inner.flow_group_repo
    }

    /// Shared `FlowPatternRepo` (ADR-017 Phase 2.A).
    pub fn flow_pattern_repo(&self) -> &Arc<FlowPatternRepo> {
        &self.inner.flow_pattern_repo
    }

    /// Shared flow-pattern matcher (ADR-017 Phase 2.B).
    pub fn flow_matcher(&self) -> Arc<RwLock<dyn FlowPatternMatcher>> {
        self.inner.flow_matcher.clone()
    }

    /// Concrete RuVector flow-pattern matcher handle (ADR-017 Phase 2.C),
    /// only populated when the `sona` feature is enabled and construction
    /// succeeded.
    #[cfg(feature = "sona")]
    pub fn flow_matcher_ruvector(&self) -> Option<Arc<RwLock<RuVectorPatternMatcher>>> {
        self.inner.flow_matcher_ruvector.clone()
    }

    pub fn ws_manager(&self) -> &WsConnectionManager {
        &self.inner.ws_manager
    }

    /// Returns the LLM client if loaded, or `None` while still loading / on failure.
    pub fn llm_client(&self) -> Option<Arc<dyn LlmClient>> {
        self.inner
            .llm_client
            .read()
            .expect("llm_client lock poisoned")
            .clone()
    }

    pub fn object_storage(&self) -> &ObjectStorage {
        &self.inner.object_storage
    }

    pub fn feed_service(&self) -> &CachedFeedService {
        &self.inner.feed_service
    }

    /// Returns the shared merchant registry for Tier 0 categorization.
    pub fn merchant_registry(&self) -> &Arc<RwLock<MerchantRegistry>> {
        &self.inner.merchant_registry
    }

    /// Returns the shared Tier 2 semantic categorizer (clone of the `Arc`).
    pub fn semantic_tier2(&self) -> Arc<RwLock<dyn SemanticCategorizer>> {
        self.inner.semantic_tier2.clone()
    }

    /// Returns the concrete RuVector Tier 2 handle if the `sona` feature is
    /// enabled and the resolved backend is RuVector.
    #[cfg(feature = "sona")]
    pub fn semantic_tier2_ruvector(&self) -> Option<Arc<RwLock<RuVectorEmbeddingStore>>> {
        self.inner.semantic_tier2_ruvector.clone()
    }

    /// Get the status of an on-demand categorization job.
    pub fn get_categorization_status(
        &self,
        user_id: Uuid,
        account_id: Uuid,
    ) -> Option<CategorizationJobStatus> {
        self.inner
            .categorization_jobs
            .read()
            .expect("categorization_jobs lock poisoned")
            .get(&(user_id, account_id))
            .cloned()
    }

    /// Set the status of an on-demand categorization job.
    pub fn set_categorization_status(
        &self,
        user_id: Uuid,
        account_id: Uuid,
        status: CategorizationJobStatus,
    ) {
        self.inner
            .categorization_jobs
            .write()
            .expect("categorization_jobs lock poisoned")
            .insert((user_id, account_id), status);
    }

    /// Update per-upload categorization progress.
    pub fn set_upload_categorization_progress(
        &self,
        upload_id: Uuid,
        categorized: usize,
        total: usize,
    ) {
        self.inner
            .upload_categorization_progress
            .write()
            .expect("upload_categorization_progress lock poisoned")
            .insert(upload_id, (categorized, total));
    }

    /// Read per-upload categorization progress.
    pub fn get_upload_categorization_progress(&self, upload_id: Uuid) -> Option<(usize, usize)> {
        self.inner
            .upload_categorization_progress
            .read()
            .expect("upload_categorization_progress lock poisoned")
            .get(&upload_id)
            .copied()
    }

    /// Remove per-upload categorization progress once done.
    pub fn clear_upload_categorization_progress(&self, upload_id: Uuid) {
        self.inner
            .upload_categorization_progress
            .write()
            .expect("upload_categorization_progress lock poisoned")
            .remove(&upload_id);
    }

    /// Signal all background tasks to stop.
    pub fn signal_shutdown(&self) {
        self.inner.shutdown.store(true, Ordering::Release);
    }

    /// Returns `true` if the application is shutting down.
    pub fn is_shutting_down(&self) -> bool {
        self.inner.shutdown.load(Ordering::Acquire)
    }

    /// Remove a completed/failed categorization job (after the client has polled it).
    pub fn clear_categorization_status(&self, user_id: Uuid, account_id: Uuid) {
        self.inner
            .categorization_jobs
            .write()
            .expect("categorization_jobs lock poisoned")
            .remove(&(user_id, account_id));
    }

    /// Returns the shared embedding provider. Always populated; the
    /// noop implementation errors cleanly for callers that check
    /// `backend() != "noop"` before invoking.
    pub fn embedder(&self) -> Arc<dyn EmbeddingProvider> {
        self.inner.embedder.clone()
    }

    /// Install a metrics registry for handler-level instrumentation.
    /// Called once from `main` after the registry is constructed.
    pub fn set_metrics(&self, registry: MetricsRegistry) {
        *self.inner.metrics.write().expect("metrics lock poisoned") = Some(registry);
    }

    /// Clone of the installed metrics registry, if any.
    pub fn metrics(&self) -> Option<MetricsRegistry> {
        self.inner
            .metrics
            .read()
            .expect("metrics lock poisoned")
            .clone()
    }
}

/// Construct the embedding provider based on YAML config and the
/// compile-time `embedder-*` feature flags. Falls back to
/// `NoopEmbedder` (logging a warning) when the requested backend's
/// feature is not enabled.
fn build_embedder(config: &AppConfig) -> Arc<dyn EmbeddingProvider> {
    let cfg = &config.embedder;
    match cfg.backend.as_str() {
        "none" | "" => {
            tracing::info!(dim = cfg.dim, "Embedder: NoopEmbedder (backend=none)");
            Arc::new(NoopEmbedder::new(cfg.dim))
        }
        "ollama" => {
            #[cfg(feature = "embedder-ollama")]
            {
                tracing::info!(
                    url = %cfg.ollama.url,
                    model = %cfg.ollama.model,
                    dim = cfg.dim,
                    "Embedder: OllamaEmbedder"
                );
                let e = finima_embed::OllamaEmbedder::new(
                    cfg.ollama.url.clone(),
                    cfg.ollama.model.clone(),
                    cfg.dim,
                )
                .with_timeout_ms(cfg.ollama.timeout_millis);
                Arc::new(e) as Arc<dyn EmbeddingProvider>
            }
            #[cfg(not(feature = "embedder-ollama"))]
            {
                tracing::warn!(
                    requested = "ollama",
                    "Embedder feature `embedder-ollama` not enabled at compile time; using NoopEmbedder"
                );
                Arc::new(NoopEmbedder::new(cfg.dim))
            }
        }
        "candle" => {
            #[cfg(feature = "embedder-candle")]
            {
                tracing::info!(
                    model_id = %cfg.candle.model_id,
                    dim = cfg.dim,
                    "Embedder: CandleEmbedder (stub)"
                );
                Arc::new(finima_embed::CandleEmbedder::new(
                    cfg.candle.model_id.clone(),
                    cfg.dim,
                )) as Arc<dyn EmbeddingProvider>
            }
            #[cfg(not(feature = "embedder-candle"))]
            {
                tracing::warn!(
                    requested = "candle",
                    "Embedder feature `embedder-candle` not enabled at compile time; using NoopEmbedder"
                );
                Arc::new(NoopEmbedder::new(cfg.dim))
            }
        }
        other => {
            tracing::warn!(
                requested = %other,
                "Unknown embedder.backend value; using NoopEmbedder"
            );
            Arc::new(NoopEmbedder::new(cfg.dim))
        }
    }
}
