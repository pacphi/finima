use std::sync::Arc;

use sqlx::PgPool;

use finima_auth::EmailSender;
use finima_db::{
    PgAccountRepo, PgBudgetRepo, PgFlowGroupRepo, PgFlowRepo, PgMagicLinkRepo, PgOverrideRepo,
    PgPortfolioRepo, PgRecurringRepo, PgSavingsGoalRepo, PgSessionRepo, PgTransactionRepo,
    PgUploadRepo, PgUserRepo,
};
use finima_llm::LlmClient;

use crate::config::AppConfig;
use crate::storage::ObjectStorage;
use crate::ws::WsConnectionManager;

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
    pub llm_client: Arc<dyn LlmClient>,
    pub object_storage: ObjectStorage,
}

impl AppState {
    pub async fn new(
        pool: PgPool,
        config: AppConfig,
        email_sender: Box<dyn EmailSender>,
        object_storage: ObjectStorage,
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

        // Build the LLM client based on configured provider.
        let llm_client: Arc<dyn LlmClient> = match config.llm.provider.as_str() {
            #[cfg(feature = "candle")]
            "candle" => {
                use finima_llm::{CandleClient, CandleConfig as LlmCandleConfig};
                let candle_cfg = LlmCandleConfig {
                    model_id: config.llm.candle.model_id.clone(),
                    model_path: config.llm.candle.model_path.clone(),
                    quantization: config.llm.candle.quantization.clone(),
                    device: config.llm.candle.device.clone(),
                    context_length: config.llm.candle.context_length,
                    threads: config.llm.candle.threads,
                };
                match CandleClient::new(candle_cfg).await {
                    Ok(client) => Arc::new(client),
                    Err(e) => {
                        tracing::warn!(
                            "Failed to initialize Candle backend: {}. Falling back to stub.",
                            e
                        );
                        Arc::new(finima_llm::StubLlmClient::new())
                    }
                }
            }
            #[cfg(not(feature = "candle"))]
            "candle" => {
                tracing::warn!(
                    "Provider is 'candle' but the candle feature is not enabled. Falling back to stub."
                );
                Arc::new(finima_llm::StubLlmClient::new())
            }
            #[cfg(feature = "ollama")]
            "ollama" if !config.llm.ollama.url.is_empty() => Arc::new(
                finima_llm::OllamaClient::new(&config.llm.ollama.url, &config.llm.ollama.model),
            ),
            #[cfg(not(feature = "ollama"))]
            "ollama" => {
                tracing::warn!(
                    "Provider is 'ollama' but the ollama feature is not enabled. Falling back to stub."
                );
                Arc::new(finima_llm::StubLlmClient::new())
            }
            _ => Arc::new(finima_llm::StubLlmClient::new()),
        };

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
                llm_client,
                object_storage,
            }),
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

    pub fn ws_manager(&self) -> &WsConnectionManager {
        &self.inner.ws_manager
    }

    pub fn llm_client(&self) -> Arc<dyn LlmClient> {
        Arc::clone(&self.inner.llm_client)
    }

    pub fn object_storage(&self) -> &ObjectStorage {
        &self.inner.object_storage
    }
}
