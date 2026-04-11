# finima-feed

RSS/Atom feed aggregation with heuristic relevance scoring and LLM-powered article summarization.

## Purpose

This crate fetches financial news from configured RSS and Atom feed sources, parses them into structured articles, scores their relevance to the user's portfolio (based on account types and spending categories), and generates concise summaries via the LLM backend. It provides the data behind the news feed feature in the Finima dashboard.

## Key Types / Modules

| Module          | Description                                                                                                                                                                                                       |
| --------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `lib.rs`        | Core types: `FeedSource` (configured feed with name, URL, topic, enabled flag), `RawArticle` (parsed article with title, URL, snippet, topics), `FeedArticle` (enriched article with summary and relevance score) |
| `fetcher.rs`    | `FeedFetcher` -- HTTP client that fetches and parses RSS/Atom feeds via the `feed-rs` crate; `parse_feed_bytes()` for testing with fixture data; includes HTML tag stripping and safe UTF-8 truncation            |
| `relevance.rs`  | `RelevanceScorer` -- heuristic 1-5 scoring based on topic-to-account-type matching, spending category mentions, educational content keywords, and direct relevance signals                                        |
| `summarizer.rs` | `ArticleSummarizer` -- delegates to `LlmClient::generate_insight()` to produce 2-sentence article summaries                                                                                                       |

## Dependencies

Depends on **finima-core** for `AccountType` (used in relevance scoring) and **finima-llm** for the `LlmClient` trait (used in summarization). Uses `feed-rs` for RSS/Atom parsing, `reqwest` for HTTP fetching, and `chrono` for date handling.

## Developer Top-of-Mind

- **Safe UTF-8 truncation**: content snippets are capped at 500 characters using `chars().take(n)`, not byte slicing, to avoid panics on multi-byte characters. Maintain this pattern for any new truncation logic.
- **Sequential feed fetching**: `fetch_all()` processes feeds sequentially in a loop. This could be parallelized with `tokio::join!` or `futures::join_all` for better performance, but has not been done yet.
- **HTML stripping** uses a simple tag-removal approach (no full HTML parser). It handles basic `<tag>` stripping and whitespace collapsing but does not decode HTML entities.
- **Relevance scoring is heuristic**: scores are computed from keyword matching against account types, spending categories, and educational keywords. The scoring rules are documented in `RelevanceScorer::score()`.
- **Summarization depends on LLM availability**: if Ollama is not configured, the `StubLlmClient` from `finima-llm` produces generic summaries. The summarizer itself is stateless -- it wraps a prompt and delegates.
- A failure in one feed source does not block others -- `fetch_all()` returns per-source results.

## Testing

```sh
cargo test -p finima-feed
```

Tests cover RSS and Atom parsing from inline XML fixtures, HTML tag stripping, content truncation, relevance scoring heuristics (base scores, topic matching, educational bonuses, cap at 5), and summarization via a mock `LlmClient`. No network access or external services required.

## Relevance Scoring Rules

| Signal           | Points | Description                                                                               |
| ---------------- | ------ | ----------------------------------------------------------------------------------------- |
| Base             | 1      | Every article starts at 1                                                                 |
| Topic match      | +1     | Article topic aligns with user account types (e.g., "investing" + InvestmentBrokerage)    |
| Category mention | +1     | Article content mentions a user's top spending category                                   |
| Educational      | +1     | Article contains keywords like "how to", "guide", "tips", "beginner"                      |
| Direct relevance | +1     | Topic directly matches specific account types (e.g., "retirement" + InvestmentRetirement) |
| Maximum          | 5      | Score is capped at 5                                                                      |

## Data Flow

1. `FeedFetcher::fetch_all()` iterates over configured `FeedSource` entries
2. Each enabled source is fetched via HTTP and parsed into `Vec<RawArticle>`
3. `RelevanceScorer::score()` assigns a 1-5 relevance score per article
4. `ArticleSummarizer::summarize()` generates a 2-sentence LLM summary on demand
5. Results are assembled into `FeedArticle` structs and returned to the API handler
