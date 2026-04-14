use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, RwLock};

use sqlx::PgPool;

use finima_auth::EmailSender;
use finima_db::{
    PgAccountRepo, PgBudgetRepo, PgFlowGroupRepo, PgFlowRepo, PgMagicLinkRepo, PgOverrideRepo,
    PgPortfolioRepo, PgRecurringRepo, PgSavingsGoalRepo, PgSessionRepo, PgTransactionRepo,
    PgUploadRepo, PgUserRepo,
};
use finima_feed::CachedFeedService;
use finima_llm::LlmClient;

use crate::config::AppConfig;
use crate::storage::ObjectStorage;
use crate::ws::WsConnectionManager;

// LLM loading status constants.
const LLM_LOADING: u8 = 0;
const LLM_READY: u8 = 1;
const LLM_FAILED: u8 = 2;

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
    pub ws_manager: WsConnectionManager,
    /// LLM client loaded in the background. `None` until the model is ready
    /// (or if loading failed).
    pub llm_client: RwLock<Option<Arc<dyn LlmClient>>>,
    /// LLM loading status: 0 = loading, 1 = ready, 2 = failed.
    pub llm_status: AtomicU8,
    pub object_storage: ObjectStorage,
    pub feed_service: CachedFeedService,
}

impl AppState {
    /// Creates application state with a stub LLM client so the server can start
    /// accepting requests immediately. Call [`spawn_llm_loader`] afterwards to
    /// load the real backend in the background.
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
                ws_manager,
                llm_client: RwLock::new(None),
                llm_status: AtomicU8::new(LLM_LOADING),
                object_storage,
                feed_service,
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

    /// LLM loading status: `"loading"`, `"ready"`, or `"failed"`.
    pub fn llm_status(&self) -> &'static str {
        match self.inner.llm_status.load(Ordering::Acquire) {
            LLM_READY => "ready",
            LLM_FAILED => "failed",
            _ => "loading",
        }
    }

    /// Returns `true` once the real LLM backend has been loaded.
    pub fn is_llm_ready(&self) -> bool {
        self.inner.llm_status.load(Ordering::Acquire) == LLM_READY
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
}
