//! Financial news feed aggregation for Finima.
//!
//! Fetches RSS/Atom feeds, provides LLM-powered summarization,
//! and relevance scoring based on user portfolio.

pub mod fetcher;
pub mod relevance;
pub mod summarizer;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A configured RSS/Atom feed source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedSource {
    pub name: String,
    pub url: String,
    pub topic: String,
    pub enabled: bool,
}

/// A raw article parsed from an RSS/Atom feed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawArticle {
    pub title: String,
    pub url: String,
    pub source_name: String,
    pub published_at: Option<DateTime<Utc>>,
    pub content_snippet: String,
    pub topics: Vec<String>,
}

/// An article enriched with summary and relevance score for API responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedArticle {
    pub id: String,
    pub title: String,
    pub url: String,
    pub source: String,
    pub date: Option<String>,
    pub summary: Option<String>,
    pub relevance_score: u8,
    pub topics: Vec<String>,
}

// Re-exports for convenience.
pub use fetcher::FeedFetcher;
pub use relevance::RelevanceScorer;
pub use summarizer::ArticleSummarizer;
