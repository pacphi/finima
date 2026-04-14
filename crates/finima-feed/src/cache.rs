//! In-memory feed cache that refreshes periodically in the background.
//!
//! Wraps `FeedFetcher` so that HTTP requests to external RSS sources happen
//! at most once per refresh interval, not on every API call.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::RwLock;
use tracing::{error, info};

use crate::{FeedFetcher, FeedSource, RawArticle};

/// Shared, time-based cache of feed articles.
///
/// The cache starts empty and populates in the background so the server
/// can begin accepting requests immediately. Call
/// [`CachedFeedService::start_background_refresh`] after construction to
/// kick off the initial fetch and periodic refreshes.
#[derive(Clone)]
pub struct CachedFeedService {
    inner: Arc<Inner>,
}

struct Inner {
    cache: RwLock<Arc<Vec<RawArticle>>>,
    fetcher: FeedFetcher,
    sources: Vec<FeedSource>,
    /// `true` once the first fetch has completed (success or failure).
    ready: AtomicBool,
}

impl CachedFeedService {
    /// Create a new cache. The cache starts empty; call
    /// [`start_background_refresh`] to begin populating it.
    pub fn new(sources: Vec<FeedSource>, fetcher: FeedFetcher) -> Self {
        Self {
            inner: Arc::new(Inner {
                cache: RwLock::new(Arc::new(Vec::new())),
                fetcher,
                sources,
                ready: AtomicBool::new(false),
            }),
        }
    }

    /// Spawn a background task that fetches feeds immediately, then
    /// refreshes every `interval`.
    pub fn start_background_refresh(&self, interval: Duration) {
        let inner = Arc::clone(&self.inner);
        tokio::spawn(async move {
            // Initial fetch.
            let articles = Self::fetch_articles(&inner.fetcher, &inner.sources).await;
            let count = articles.len();
            *inner.cache.write().await = Arc::new(articles);
            inner.ready.store(true, Ordering::Release);
            info!(count, "Initial feed cache populated");

            // Periodic refresh.
            loop {
                tokio::time::sleep(interval).await;
                let articles = Self::fetch_articles(&inner.fetcher, &inner.sources).await;
                let count = articles.len();
                *inner.cache.write().await = Arc::new(articles);
                info!(count, "Feed cache refreshed");
            }
        });
    }

    /// Returns `true` once the initial fetch has completed.
    pub fn is_ready(&self) -> bool {
        self.inner.ready.load(Ordering::Acquire)
    }

    /// Get a snapshot of all cached articles (cheap `Arc` clone).
    pub async fn articles(&self) -> Arc<Vec<RawArticle>> {
        Arc::clone(&*self.inner.cache.read().await)
    }

    async fn fetch_articles(fetcher: &FeedFetcher, sources: &[FeedSource]) -> Vec<RawArticle> {
        let results = fetcher.fetch_all(sources).await;
        let mut articles = Vec::new();
        for (source, result) in results {
            match result {
                Ok(items) => {
                    info!(source = %source, count = items.len(), "Feed fetched");
                    articles.extend(items);
                }
                Err(e) => {
                    error!(source = %source, error = %e, "Feed fetch failed");
                }
            }
        }
        // Sort newest first.
        articles.sort_by(|a, b| b.published_at.cmp(&a.published_at));
        articles
    }
}
