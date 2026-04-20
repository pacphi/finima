import { useState, useEffect } from 'react';
import { useApi } from '@/hooks/useApi';
import { useHealthStore } from '@/stores/healthStore';

interface SummaryResponse {
  article_id: string;
  summary: string;
}

// ── Types ───────────────────────────────────────────────────────────

interface FeedArticle {
  id: string;
  title: string;
  url: string;
  source: string;
  date: string | null;
  summary: string | null;
  relevance_score: number;
  topics: string[];
}

interface FeedResponse {
  data: FeedArticle[];
  total: number;
  page: number;
  per_page: number;
}

const TOPICS = [
  'All',
  'Budgeting',
  'Investing',
  'Taxes',
  'Credit',
  'Retirement',
  'Real Estate',
  'Economy',
];
const PER_PAGE = 20;

// ── Component ───────────────────────────────────────────────────────

export function NewsPage() {
  const api = useApi();
  const feedStatus = useHealthStore((s) => s.feedStatus);
  const [articles, setArticles] = useState<FeedArticle[]>([]);
  const [total, setTotal] = useState(0);
  const [page, setPage] = useState(1);
  const [activeTopic, setActiveTopic] = useState('All');
  const [loading, setLoading] = useState(false);

  // Fetch on page/topic change, and re-fetch once when the feed cache becomes ready.
  // `api` is stable (useApi returns a memoized object), so including it in deps is safe.
  useEffect(() => {
    let ignore = false;
    (async () => {
      try {
        const topicParam = activeTopic === 'All' ? '' : `&topic=${activeTopic.toLowerCase()}`;
        const result = await api.get<FeedResponse>(
          `/api/feed?page=${page}&per_page=${PER_PAGE}${topicParam}`,
        );
        if (ignore) return;
        setArticles(result.data);
        setTotal(result.total);
      } catch (err) {
        if (!ignore) console.error('Failed to fetch feed:', err);
      } finally {
        if (!ignore) setLoading(false);
      }
    })();
    return () => {
      ignore = true;
    };
  }, [api, page, activeTopic, feedStatus]);

  const handleTopicChange = (topic: string) => {
    setArticles([]);
    setActiveTopic(topic);
    setPage(1);
  };

  const totalPages = Math.ceil(total / PER_PAGE);

  return (
    <div className="p-6">
      <h1 className="text-2xl font-bold text-[var(--color-text)] mb-6">Financial News</h1>

      {/* Topic filter tabs */}
      <div className="flex flex-wrap gap-2 mb-6" role="group" aria-label="Filter by topic">
        {TOPICS.map((topic) => (
          <button
            key={topic}
            onClick={() => handleTopicChange(topic)}
            aria-pressed={activeTopic === topic}
            className={`px-4 py-2 text-sm rounded-lg transition-colors ${
              activeTopic === topic
                ? 'bg-[var(--color-primary)] text-white font-medium'
                : 'bg-[var(--color-surface)] text-[var(--color-text-secondary)] border border-[var(--color-border)] hover:bg-[var(--color-border)]'
            }`}
          >
            {topic}
          </button>
        ))}
      </div>

      {/* Feed cache still loading */}
      {feedStatus === 'loading' && articles.length === 0 && (
        <div className="text-center py-12">
          <div className="inline-block h-6 w-6 animate-spin rounded-full border-2 border-[var(--color-text-secondary)] border-t-transparent mb-3" />
          <p className="text-[var(--color-text-secondary)]">
            Fetching news feeds from {TOPICS.length - 1} sources&hellip;
          </p>
          <p className="text-xs text-[var(--color-text-secondary)] mt-1">
            This may take a few seconds on first load.
          </p>
        </div>
      )}

      {/* API request in progress (cache is ready but page/filter changed) */}
      {loading && feedStatus === 'ready' && articles.length === 0 && (
        <div className="text-center py-12 text-[var(--color-text-secondary)]">
          Loading articles...
        </div>
      )}

      {/* Empty state — cache is ready but no articles match */}
      {!loading && feedStatus === 'ready' && articles.length === 0 && (
        <div className="text-center py-12">
          <p className="text-[var(--color-text-secondary)]">No articles found for this topic.</p>
        </div>
      )}

      {/* Article card grid */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
        {articles.map((article) => (
          <ArticleCard key={article.id} article={article} api={api} />
        ))}
      </div>

      {/* Pagination */}
      {totalPages > 1 && (
        <div className="flex items-center justify-center gap-2 mt-8">
          <button
            onClick={() => setPage((p) => Math.max(1, p - 1))}
            disabled={page <= 1}
            className="px-3 py-1.5 text-sm border border-[var(--color-border)] rounded-lg disabled:opacity-50 hover:bg-[var(--color-surface)] text-[var(--color-text)]"
          >
            Previous
          </button>
          <span className="text-sm text-[var(--color-text-secondary)]">
            Page {page} of {totalPages}
          </span>
          <button
            onClick={() => setPage((p) => Math.min(totalPages, p + 1))}
            disabled={page >= totalPages}
            className="px-3 py-1.5 text-sm border border-[var(--color-border)] rounded-lg disabled:opacity-50 hover:bg-[var(--color-surface)] text-[var(--color-text)]"
          >
            Next
          </button>
        </div>
      )}
    </div>
  );
}

// ── Article Card ────────────────────────────────────────────────────

function ArticleCard({
  article,
  api,
}: {
  article: FeedArticle;
  api: { get: <T>(path: string) => Promise<T> };
}) {
  const [llmSummary, setLlmSummary] = useState<string | null>(null);
  const [loadingSummary, setLoadingSummary] = useState(false);

  const formattedDate = article.date
    ? new Date(article.date).toLocaleDateString('en-US', {
        month: 'short',
        day: 'numeric',
        year: 'numeric',
      })
    : null;

  const displaySummary = llmSummary ?? article.summary;

  const handleSummarize = async (e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setLoadingSummary(true);
    try {
      const result = await api.get<SummaryResponse>(
        `/api/feed/${encodeURIComponent(article.id)}/summary`,
      );
      setLlmSummary(result.summary);
    } catch (err) {
      console.error('Failed to generate summary:', err);
    } finally {
      setLoadingSummary(false);
    }
  };

  return (
    <a
      href={article.url}
      target="_blank"
      rel="noopener noreferrer"
      className="block p-5 rounded-xl border border-[var(--color-border)] bg-[var(--color-card)] hover:border-[var(--color-primary)] transition-colors group"
    >
      {/* Source badge + date */}
      <div className="flex items-center justify-between mb-3">
        <span className="inline-block px-2 py-0.5 text-xs font-medium rounded-full bg-[var(--color-surface)] text-[var(--color-text-secondary)] border border-[var(--color-border)]">
          {article.source}
        </span>
        {formattedDate && (
          <span className="text-xs text-[var(--color-text-secondary)]">{formattedDate}</span>
        )}
      </div>

      {/* Title */}
      <h3 className="font-semibold text-[var(--color-text)] mb-2 line-clamp-2 group-hover:text-[var(--color-primary)] transition-colors">
        {article.title}
      </h3>

      {/* Summary (2-line clamp) */}
      {displaySummary && (
        <p
          className={`text-sm text-[var(--color-text-secondary)] mb-3 ${llmSummary ? '' : 'line-clamp-2'}`}
        >
          {displaySummary}
        </p>
      )}

      {/* AI Summarize button */}
      {!llmSummary && (
        <button
          onClick={handleSummarize}
          disabled={loadingSummary}
          className="inline-flex items-center gap-1 text-xs text-[var(--color-primary)] hover:underline mb-3 disabled:opacity-50"
        >
          {loadingSummary ? (
            <>
              <svg
                className="w-3 h-3 animate-spin"
                viewBox="0 0 16 16"
                fill="none"
                aria-hidden="true"
              >
                <circle
                  cx="8"
                  cy="8"
                  r="6"
                  stroke="currentColor"
                  strokeWidth="2"
                  strokeDasharray="28"
                  strokeDashoffset="8"
                />
              </svg>
              Summarizing&hellip;
            </>
          ) : (
            <>
              <svg
                className="w-3.5 h-3.5"
                viewBox="0 0 20 20"
                fill="currentColor"
                aria-hidden="true"
              >
                <path
                  fillRule="evenodd"
                  d="M4 4a2 2 0 012-2h4.586A2 2 0 0112 2.586L15.414 6A2 2 0 0116 7.414V16a2 2 0 01-2 2H6a2 2 0 01-2-2V4zm2 6a1 1 0 011-1h6a1 1 0 110 2H7a1 1 0 01-1-1zm1 3a1 1 0 100 2h6a1 1 0 100-2H7z"
                  clipRule="evenodd"
                />
              </svg>
              Get Summary
            </>
          )}
        </button>
      )}

      {/* Relevance stars + topics */}
      <div className="flex items-center justify-between">
        <RelevanceStars score={article.relevance_score} />
        <div className="flex gap-1">
          {article.topics.slice(0, 2).map((topic) => (
            <span
              key={topic}
              className="text-xs text-[var(--color-text-secondary)] bg-[var(--color-surface)] px-1.5 py-0.5 rounded"
            >
              {topic}
            </span>
          ))}
        </div>
      </div>
    </a>
  );
}

// ── Relevance Stars ─────────────────────────────────────────────────

function RelevanceStars({ score }: { score: number }) {
  const clamped = Math.max(1, Math.min(5, score));
  return (
    <div
      className="flex items-center gap-0.5"
      role="img"
      aria-label={`Relevance: ${clamped} out of 5 stars`}
    >
      {Array.from({ length: 5 }, (_, i) => (
        <svg
          key={i}
          className={`w-4 h-4 ${i < clamped ? 'text-yellow-400' : 'text-[var(--color-border)]'}`}
          fill="currentColor"
          viewBox="0 0 20 20"
          aria-hidden="true"
        >
          <path d="M9.049 2.927c.3-.921 1.603-.921 1.902 0l1.07 3.292a1 1 0 00.95.69h3.462c.969 0 1.371 1.24.588 1.81l-2.8 2.034a1 1 0 00-.364 1.118l1.07 3.292c.3.921-.755 1.688-1.54 1.118l-2.8-2.034a1 1 0 00-1.175 0l-2.8 2.034c-.784.57-1.838-.197-1.539-1.118l1.07-3.292a1 1 0 00-.364-1.118L2.98 8.72c-.783-.57-.38-1.81.588-1.81h3.461a1 1 0 00.951-.69l1.07-3.292z" />
        </svg>
      ))}
    </div>
  );
}
