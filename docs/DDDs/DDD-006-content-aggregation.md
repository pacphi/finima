# DDD-006: Content Aggregation Bounded Context

**Date:** 2026-04-10  
**Crate:** `finima-feed`

---

## 1. Purpose

Fetches, parses, and enriches financial news and educational content from external RSS/Atom feeds. Provides LLM-powered summarization and relevance scoring based on the user's portfolio composition.

## 2. Ubiquitous Language

| Term                | Definition                                                                                                                                       |
| ------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------ |
| **Feed Source**     | A configured RSS or Atom feed URL from a financial content provider.                                                                             |
| **Article**         | A single item from a feed: title, source, date, URL, and content/description.                                                                    |
| **Summary**         | A 2-sentence LLM-generated summary of an article's key points.                                                                                   |
| **Relevance Score** | A 1-5 rating of how relevant an article is to the user's financial situation (based on portfolio composition: account types, spending patterns). |
| **Topic**           | A classification tag: budgeting, investing, taxes, credit, retirement. Used for filtering.                                                       |

## 3. Aggregates

### FeedSource (Configuration, not a DB entity initially)

```text
FeedSource
  url: String
  name: String (e.g., "Investopedia", "NerdWallet")
  topic: Topic (enum)
  enabled: bool
```

Default sources are configured in application config. Users may add/remove sources in a future iteration.

### Article (Entity, cached)

```text
Article
  id: UUID (hash of url)
  feed_source: String
  title: String
  url: String (external link)
  published_at: DateTime
  content_snippet: String (first 500 chars of article body)
  summary: String? (LLM-generated, populated lazily)
  relevance_score: u8? (1-5, populated lazily per user)
  topics: Vec<Topic>
```

**Invariants:**

- Articles are deduplicated by URL hash.
- Summaries are generated on first access (lazy) or in a background batch.
- Relevance scores are per-user (different portfolio compositions yield different scores). Stored in a join table or computed on-the-fly.

## 4. Domain Services

### FeedFetcher

- `fetch_all() -> Result<Vec<RawArticle>>` — Fetch and parse all configured RSS/Atom feeds using `feed-rs` crate. Deduplicate by URL.
- `fetch_source(url) -> Result<Vec<RawArticle>>` — Fetch a single source.

### ArticleSummarizer

- `summarize(article) -> Result<String>` — Send article title + content snippet to LLM, request 2-sentence summary.
- Uses `LlmClient` from the Intelligence context (shared dependency).

### RelevanceScorer

- `score(article, user_portfolio) -> Result<u8>` — Evaluate article relevance based on:
  - User's account types (has investment accounts? retirement? loans?).
  - User's top spending categories.
  - Article topics.
  - Simple heuristic scoring (no LLM needed for v1; LLM-powered for v2).

## 5. Context Boundaries

**This context consumes from other contexts:**

- `LlmClient` from Intelligence context for summarization.
- User portfolio composition (account types, spending categories) from Portfolio Management and Financial Analysis contexts.

**This context provides to other contexts:**

- Paginated article feed for the News/Learn page.
- Article summaries and relevance scores for display.

**This context does NOT know about:**

- Transactions, budgets, or flows.
- Authentication details (receives `user_id` from API layer).

## 6. Key Design Decisions

- **Lazy summarization:** Summaries are generated on first request rather than batch-processing all articles. This avoids wasting LLM compute on articles no one reads.
- **Cached locally:** Articles are stored in the database to avoid re-fetching. Feed polling runs on a configurable interval (default: every 6 hours).
- **No content hosting:** Finima links to original articles. Only title, snippet, and summary are stored. No copyright concerns.
- **Default sources curated for quality:** Investopedia, NerdWallet, The Motley Fool educational content, Federal Reserve data summaries. Biased toward education, not stock tips.
