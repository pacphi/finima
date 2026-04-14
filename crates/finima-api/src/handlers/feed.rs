use axum::extract::{Path, Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};

use finima_auth::middleware::AuthUser;
use finima_core::AppError;
use finima_feed::{FeedArticle, RawArticle, RelevanceScorer};

use crate::state::AppState;

// ---------------------------------------------------------------------------
// Request / response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct FeedQuery {
    #[serde(default = "default_page")]
    pub page: u32,
    #[serde(default = "default_per_page")]
    pub per_page: u32,
    pub topic: Option<String>,
}

fn default_page() -> u32 {
    1
}
fn default_per_page() -> u32 {
    20
}

#[derive(Debug, Serialize)]
pub struct FeedResponse {
    pub data: Vec<FeedArticle>,
    pub total: usize,
    pub page: u32,
    pub per_page: u32,
}

#[derive(Debug, Serialize)]
pub struct SummaryResponse {
    pub article_id: String,
    pub summary: String,
}

// ---------------------------------------------------------------------------
// GET /api/feed
// ---------------------------------------------------------------------------

pub async fn list_feed(
    _auth: AuthUser,
    State(state): State<AppState>,
    Query(params): Query<FeedQuery>,
) -> Result<Json<FeedResponse>, AppError> {
    // Read from the in-memory cache (pre-sorted by date, refreshed in background).
    let cached = state.feed_service().articles().await;

    // Apply topic filter if specified.
    let filtered: Vec<&RawArticle> = if let Some(ref topic) = params.topic {
        let lower_topic = topic.to_lowercase();
        cached
            .iter()
            .filter(|a| a.topics.iter().any(|t| t.to_lowercase() == lower_topic))
            .collect()
    } else {
        cached.iter().collect()
    };

    let total = filtered.len();

    // Paginate.
    let start = ((params.page - 1) * params.per_page) as usize;
    let page_articles: Vec<_> = filtered
        .into_iter()
        .skip(start)
        .take(params.per_page as usize)
        .collect();

    // Score and convert to response articles.
    let feed_articles: Vec<FeedArticle> = page_articles
        .into_iter()
        .enumerate()
        .map(|(i, raw)| {
            let score = RelevanceScorer::score(raw, &[], &[]);
            FeedArticle {
                id: format!("article-{}-{}", params.page, i),
                title: raw.title.clone(),
                url: raw.url.clone(),
                source: raw.source_name.clone(),
                date: raw.published_at.map(|d| d.to_rfc3339()),
                summary: None,
                relevance_score: score,
                topics: raw.topics.clone(),
            }
        })
        .collect();

    Ok(Json(FeedResponse {
        data: feed_articles,
        total,
        page: params.page,
        per_page: params.per_page,
    }))
}

// ---------------------------------------------------------------------------
// GET /api/feed/:id/summary
// ---------------------------------------------------------------------------

pub async fn get_article_summary(
    _auth: AuthUser,
    State(state): State<AppState>,
    Path(article_id): Path<String>,
) -> Result<Json<SummaryResponse>, AppError> {
    let llm_client = state.llm_client().ok_or_else(|| {
        AppError::ServiceUnavailable(
            "LLM backend is still loading — please try again shortly".to_string(),
        )
    })?;

    // Parse the article ID to extract page and index.
    let parts: Vec<&str> = article_id.split('-').collect();
    let (page, index) = match parts.as_slice() {
        ["article", page_str, index_str] => {
            let page: u32 = page_str
                .parse()
                .map_err(|_| AppError::BadRequest("Invalid article ID format".to_string()))?;
            let index: usize = index_str
                .parse()
                .map_err(|_| AppError::BadRequest("Invalid article ID format".to_string()))?;
            (page, index)
        }
        _ => {
            return Err(AppError::BadRequest(
                "Invalid article ID format".to_string(),
            ))
        }
    };

    // Look up the article from the cache (same ordering as list_feed).
    let cached = state.feed_service().articles().await;
    let per_page: u32 = 20;
    let start = ((page - 1) * per_page) as usize;
    let absolute_index = start + index;

    let article = cached.get(absolute_index).ok_or(AppError::NotFound)?;

    let summary = finima_feed::ArticleSummarizer::summarize(
        llm_client.as_ref(),
        &article.title,
        &article.content_snippet,
    )
    .await
    .map_err(|e| AppError::InternalError(e.to_string()))?;

    Ok(Json(SummaryResponse {
        article_id,
        summary,
    }))
}
