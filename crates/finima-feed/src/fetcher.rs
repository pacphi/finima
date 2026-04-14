//! RSS/Atom feed fetching and parsing.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use feed_rs::parser;
use reqwest::Url;
use tracing::{error, info};

use crate::{FeedSource, RawArticle};

/// Errors that can occur during feed fetching.
#[derive(Debug, thiserror::Error)]
pub enum FetchError {
    #[error("HTTP error: {0}")]
    Http(String),

    #[error("Parse error: {0}")]
    Parse(String),
}

/// Fetches and parses RSS/Atom feeds into `RawArticle`s.
pub struct FeedFetcher {
    client: reqwest::Client,
}

impl FeedFetcher {
    /// Create a new `FeedFetcher` with a default HTTP client.
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(15))
                .user_agent("Finima/1.0 (RSS Reader; +https://github.com/pacphi/finima)")
                .build()
                .unwrap_or_default(),
        }
    }

    /// Create a `FeedFetcher` with a custom `reqwest::Client`.
    pub fn with_client(client: reqwest::Client) -> Self {
        Self { client }
    }

    /// Fetch and parse a single RSS/Atom feed URL.
    pub async fn fetch_from_url(
        &self,
        url: &str,
        source_name: &str,
        topic: &str,
    ) -> Result<Vec<RawArticle>, FetchError> {
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| FetchError::Http(e.to_string()))?;

        let bytes = response
            .bytes()
            .await
            .map_err(|e| FetchError::Http(e.to_string()))?;

        let feed = parser::parse(&bytes[..]).map_err(|e| FetchError::Parse(e.to_string()))?;

        let articles = feed
            .entries
            .into_iter()
            .map(|entry| {
                let title = entry
                    .title
                    .map(|t| t.content)
                    .unwrap_or_else(|| "Untitled".to_string());

                let url = entry
                    .links
                    .first()
                    .map(|l| l.href.clone())
                    .unwrap_or_default();

                let published_at: Option<DateTime<Utc>> = entry.published.or(entry.updated);

                // Extract content snippet: first 500 chars of the body or summary.
                let raw_content = entry
                    .content
                    .and_then(|c| c.body)
                    .or_else(|| entry.summary.map(|s| s.content))
                    .unwrap_or_default();

                // Strip HTML tags for a clean snippet.
                let clean = strip_html_tags(&raw_content);
                let content_snippet = truncate_to_char_boundary(&clean, 500);

                RawArticle {
                    title,
                    url,
                    source_name: source_name.to_string(),
                    published_at,
                    content_snippet,
                    topics: vec![topic.to_string()],
                }
            })
            .collect();

        Ok(articles)
    }

    /// Fetch all configured feed sources with per-host rate limiting.
    ///
    /// Feeds are grouped by host. Different hosts are fetched in parallel,
    /// but feeds sharing a host are fetched sequentially with a 1-second
    /// delay between requests to avoid triggering rate limits.
    pub async fn fetch_all(
        &self,
        sources: &[FeedSource],
    ) -> Vec<(String, Result<Vec<RawArticle>, FetchError>)> {
        // Group sources by host so we can throttle per-host.
        let mut by_host: HashMap<String, Vec<&FeedSource>> = HashMap::new();
        for source in sources.iter().filter(|s| s.enabled) {
            let host = Url::parse(&source.url)
                .ok()
                .and_then(|u| u.host_str().map(String::from))
                .unwrap_or_else(|| "unknown".to_string());
            by_host.entry(host).or_default().push(source);
        }

        // For each host group, fetch sequentially with a delay; run host
        // groups in parallel.
        let host_futures: Vec<_> = by_host
            .into_values()
            .map(|group| async move {
                let mut results = Vec::with_capacity(group.len());
                for (i, source) in group.iter().enumerate() {
                    // Delay between requests to the same host (skip first).
                    if i > 0 {
                        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                    }
                    info!(source = %source.name, url = %source.url, "Fetching feed");
                    let result = self
                        .fetch_from_url(&source.url, &source.name, &source.topic)
                        .await;
                    if let Err(ref e) = result {
                        error!(source = %source.name, error = %e, "Failed to fetch feed");
                    }
                    results.push((source.name.clone(), result));
                }
                results
            })
            .collect();

        futures::future::join_all(host_futures)
            .await
            .into_iter()
            .flatten()
            .collect()
    }
}

impl Default for FeedFetcher {
    fn default() -> Self {
        Self::new()
    }
}

/// Very simple HTML tag stripper (no dependency on a full HTML parser).
fn strip_html_tags(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut in_tag = false;

    for ch in input.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => output.push(ch),
            _ => {}
        }
    }

    // Collapse whitespace.
    let collapsed: String = output.split_whitespace().collect::<Vec<_>>().join(" ");

    collapsed
}

/// Parse raw XML bytes into `RawArticle`s. Useful for testing with fixture strings.
pub fn parse_feed_bytes(
    bytes: &[u8],
    source_name: &str,
    topic: &str,
) -> Result<Vec<RawArticle>, FetchError> {
    let feed = parser::parse(bytes).map_err(|e| FetchError::Parse(e.to_string()))?;

    let articles = feed
        .entries
        .into_iter()
        .map(|entry| {
            let title = entry
                .title
                .map(|t| t.content)
                .unwrap_or_else(|| "Untitled".to_string());

            let url = entry
                .links
                .first()
                .map(|l| l.href.clone())
                .unwrap_or_default();

            let published_at: Option<DateTime<Utc>> = entry.published.or(entry.updated);

            let raw_content = entry
                .content
                .and_then(|c| c.body)
                .or_else(|| entry.summary.map(|s| s.content))
                .unwrap_or_default();

            let clean = strip_html_tags(&raw_content);
            let content_snippet = truncate_to_char_boundary(&clean, 500);

            RawArticle {
                title,
                url,
                source_name: source_name.to_string(),
                published_at,
                content_snippet,
                topics: vec![topic.to_string()],
            }
        })
        .collect();

    Ok(articles)
}

/// Truncate a string to at most `max_chars` characters, avoiding panics on
/// multi-byte UTF-8 by operating on character boundaries.
fn truncate_to_char_boundary(s: &str, max_chars: usize) -> String {
    s.chars().take(max_chars).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RSS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
    <rss version="2.0">
      <channel>
        <title>Finance News</title>
        <link>https://example.com</link>
        <description>Latest finance articles</description>
        <item>
          <title>How to Budget in 2026</title>
          <link>https://example.com/budget-2026</link>
          <description>Learn the basics of budgeting for the new year with these simple tips.</description>
          <pubDate>Thu, 01 Jan 2026 12:00:00 GMT</pubDate>
        </item>
        <item>
          <title>Investment Strategies for Beginners</title>
          <link>https://example.com/invest-101</link>
          <description>&lt;p&gt;Getting started with &lt;b&gt;investing&lt;/b&gt; can be daunting.&lt;/p&gt;</description>
          <pubDate>Fri, 02 Jan 2026 08:00:00 GMT</pubDate>
        </item>
      </channel>
    </rss>"#;

    const SAMPLE_ATOM: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
    <feed xmlns="http://www.w3.org/2005/Atom">
      <title>Finance Atom Feed</title>
      <entry>
        <title>Tax Season Tips</title>
        <link href="https://example.com/tax-tips"/>
        <summary>File your taxes early to avoid penalties and get your refund faster.</summary>
        <updated>2026-03-15T10:00:00Z</updated>
      </entry>
    </feed>"#;

    #[test]
    fn parse_rss_feed_returns_articles() {
        let articles =
            parse_feed_bytes(SAMPLE_RSS.as_bytes(), "Finance News", "budgeting").unwrap();

        assert_eq!(articles.len(), 2);
        assert_eq!(articles[0].title, "How to Budget in 2026");
        assert_eq!(articles[0].url, "https://example.com/budget-2026");
        assert_eq!(articles[0].source_name, "Finance News");
        assert!(!articles[0].content_snippet.is_empty());
        assert_eq!(articles[0].topics, vec!["budgeting"]);
    }

    #[test]
    fn parse_atom_feed_returns_articles() {
        let articles = parse_feed_bytes(SAMPLE_ATOM.as_bytes(), "Atom Source", "taxes").unwrap();

        assert_eq!(articles.len(), 1);
        assert_eq!(articles[0].title, "Tax Season Tips");
        assert_eq!(articles[0].url, "https://example.com/tax-tips");
        assert!(articles[0].published_at.is_some());
    }

    #[test]
    fn html_tags_are_stripped_from_content() {
        let articles = parse_feed_bytes(SAMPLE_RSS.as_bytes(), "Test", "investing").unwrap();

        // Second article has HTML in description.
        let snippet = &articles[1].content_snippet;
        assert!(!snippet.contains("<p>"));
        assert!(!snippet.contains("<b>"));
        assert!(snippet.contains("investing"));
    }

    #[test]
    fn content_snippet_is_capped_at_500_chars() {
        let long_desc = "a".repeat(1000);
        let xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
            <rss version="2.0">
              <channel>
                <title>Test</title>
                <item>
                  <title>Long Article</title>
                  <link>https://example.com/long</link>
                  <description>{}</description>
                </item>
              </channel>
            </rss>"#,
            long_desc
        );

        let articles = parse_feed_bytes(xml.as_bytes(), "Test", "general").unwrap();

        assert_eq!(articles[0].content_snippet.len(), 500);
    }

    #[test]
    fn invalid_xml_returns_parse_error() {
        let result = parse_feed_bytes(b"not xml at all", "Test", "general");
        assert!(result.is_err());
        match result.unwrap_err() {
            FetchError::Parse(_) => {} // expected
            other => panic!("Expected Parse error, got {:?}", other),
        }
    }

    #[test]
    fn strip_html_tags_basic() {
        assert_eq!(strip_html_tags("<p>Hello <b>world</b></p>"), "Hello world");
        assert_eq!(strip_html_tags("No tags here"), "No tags here");
        assert_eq!(strip_html_tags(""), "");
    }
}
