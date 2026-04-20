import { useState, useEffect, useCallback, useMemo, useRef } from 'react';
import { useSearchParams } from 'react-router-dom';
import { type SortingState } from '@tanstack/react-table';
import { TransactionTable } from '@/components/tables/TransactionTable';
import { useApi } from '@/hooks/useApi';
import { createTransactionApi } from '@/api/transactions';
import { createAccountApi } from '@/api/accounts';
import { usePortfolioStore } from '@/stores/portfolioStore';
import { useCategories, categoryLabel } from '@/hooks/useCategories';
import type {
  Transaction,
  TransactionFilters,
  PaginatedResponse,
  Account,
  CategoryCount,
} from '@/types/models';

const PER_PAGE = 30;

export function TransactionsPage() {
  const api = useApi();
  const txApi = createTransactionApi(api);
  const accountApi = createAccountApi(api);
  const { categories, categoryMap } = useCategories();
  const activePortfolioId = usePortfolioStore((s) => s.activePortfolioId);
  const storedAccounts = usePortfolioStore((s) => s.accounts);

  const [transactions, setTransactions] = useState<Transaction[]>([]);
  const [total, setTotal] = useState(0);
  const [sorting, setSorting] = useState<SortingState>([{ id: 'date', desc: true }]);
  // Filters that the user edits directly (search/category pickers/etc.). The
  // active portfolio and URL deep-link parameters are merged on top during
  // render so we never duplicate that state into an effect.
  const [userFilters, setUserFilters] = useState<TransactionFilters>({});
  const [fetchedAccounts, setFetchedAccounts] = useState<Account[]>([]);
  const accounts = storedAccounts.length > 0 ? storedAccounts : fetchedAccounts;
  const [loading, setLoading] = useState(false);
  const [searchQuery, setSearchQuery] = useState('');
  const [searchResults, setSearchResults] = useState<Transaction[] | null>(null);
  const [searching, setSearching] = useState(false);

  // Categorization state
  const [categorizing, setCategorizing] = useState(false);
  const [categorizationSummary, setCategorizationSummary] = useState<{
    total: number;
    categories: CategoryCount[];
  } | null>(null);
  const categorizePollRef = useRef<ReturnType<typeof setInterval> | null>(null);

  // Deep-link support: ?category=&date_from=&date_to=&account_id= merged from
  // the URL into `filters` during render (no state duplication, no effect).
  const [searchParams, setSearchParams] = useSearchParams();
  const urlFilters = useMemo<TransactionFilters>(() => {
    const f: TransactionFilters = {};
    const category = searchParams.get('category');
    const date_from = searchParams.get('date_from');
    const date_to = searchParams.get('date_to');
    const account_id = searchParams.get('account_id');
    if (category != null) f.category = category;
    if (date_from != null) f.date_from = date_from;
    if (date_to != null) f.date_to = date_to;
    if (account_id != null) f.account_id = account_id;
    return f;
  }, [searchParams]);
  const urlKey = useMemo(
    () =>
      ['category', 'date_from', 'date_to', 'account_id']
        .map((k) => searchParams.get(k) ?? '')
        .join('|'),
    [searchParams],
  );

  // Page lives alongside the URL key so it resets whenever the deep-link
  // filters change, without an effect that would sync-update state.
  const [pageState, setPageState] = useState<{ page: number; key: string }>({
    page: 1,
    key: urlKey,
  });
  const page = pageState.key === urlKey ? pageState.page : 1;
  const setPage = useCallback(
    (next: number | ((p: number) => number)) =>
      setPageState((prev) => {
        const basePage = prev.key === urlKey ? prev.page : 1;
        const resolved = typeof next === 'function' ? next(basePage) : next;
        return { page: resolved, key: urlKey };
      }),
    [urlKey],
  );

  // URL is the source of truth for deep-linkable filter keys (category,
  // date_from, date_to, account_id). Non-URL keys live in userFilters.
  const filters = useMemo<TransactionFilters>(
    () => ({
      ...userFilters,
      ...urlFilters,
      ...(activePortfolioId ? { portfolio_id: activePortfolioId } : {}),
    }),
    [userFilters, urlFilters, activePortfolioId],
  );

  // Fetch accounts from the API only if the portfolio store hasn't cached them.
  useEffect(() => {
    if (storedAccounts.length > 0 || !activePortfolioId) return;
    let ignore = false;
    accountApi
      .listAccounts(activePortfolioId)
      .then((a) => {
        if (!ignore) setFetchedAccounts(a);
      })
      .catch(console.error);
    return () => {
      ignore = true;
    };
  }, [activePortfolioId, storedAccounts]); // eslint-disable-line react-hooks/exhaustive-deps

  const fetchTransactions = useCallback(async () => {
    // The backend requires at least portfolio_id or account_id.
    if (!filters.portfolio_id && !filters.account_id) return;

    try {
      const sortCol = sorting[0];
      const result = await txApi.listTransactions(
        filters,
        { page, per_page: PER_PAGE },
        sortCol ? { sort_by: sortCol.id, sort_dir: sortCol.desc ? 'desc' : 'asc' } : undefined,
      );
      const response = result as PaginatedResponse<Transaction>;
      setTransactions(response.data);
      setTotal(response.total);
    } catch (err) {
      console.error('Failed to load transactions:', err);
    } finally {
      setLoading(false);
    }
  }, [filters, page, sorting]); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    if (!filters.portfolio_id && !filters.account_id) return;
    let ignore = false;
    (async () => {
      try {
        const sortCol = sorting[0];
        const result = await txApi.listTransactions(
          filters,
          { page, per_page: PER_PAGE },
          sortCol ? { sort_by: sortCol.id, sort_dir: sortCol.desc ? 'desc' : 'asc' } : undefined,
        );
        if (ignore) return;
        const response = result as PaginatedResponse<Transaction>;
        setTransactions(response.data);
        setTotal(response.total);
      } catch (err) {
        if (!ignore) console.error('Failed to load transactions:', err);
      } finally {
        if (!ignore) setLoading(false);
      }
    })();
    return () => {
      ignore = true;
    };
  }, [filters, page, sorting]); // eslint-disable-line react-hooks/exhaustive-deps

  const handleCategoryChange = async (
    transactionId: string,
    category: string,
    subcategory?: string,
  ) => {
    try {
      await txApi.updateTransaction(transactionId, { category, subcategory });
      setTransactions((prev) =>
        prev.map((t) =>
          t.id === transactionId
            ? { ...t, category, subcategory: subcategory ?? null, user_overridden: true }
            : t,
        ),
      );
    } catch (err) {
      console.error('Failed to update category:', err);
    }
  };

  const handleBulkCategoryChange = async (
    ids: string[],
    category: string,
    subcategory?: string,
  ) => {
    try {
      await txApi.bulkUpdateTransactions({ ids, category, subcategory });
      setTransactions((prev) =>
        prev.map((t) =>
          ids.includes(t.id)
            ? { ...t, category, subcategory: subcategory ?? null, user_overridden: true }
            : t,
        ),
      );
    } catch (err) {
      console.error('Failed to bulk update:', err);
    }
  };

  const handleSearch = useCallback(async () => {
    if (!searchQuery.trim() || !activePortfolioId) {
      setSearchResults(null);
      return;
    }
    setSearching(true);
    try {
      const results = await txApi.searchTransactions(searchQuery.trim(), activePortfolioId);
      setSearchResults(results);
    } catch (err) {
      console.error('Search failed:', err);
    } finally {
      setSearching(false);
    }
  }, [searchQuery, activePortfolioId]); // eslint-disable-line react-hooks/exhaustive-deps

  // Clean up categorization polling on unmount
  useEffect(() => {
    return () => {
      if (categorizePollRef.current) clearInterval(categorizePollRef.current);
    };
  }, []);

  const handleCategorize = async () => {
    // Use the first account filter, or the first account in the list
    const accountId = filters.account_id ?? accounts[0]?.id;
    if (!accountId) return;

    setCategorizing(true);
    setCategorizationSummary(null);

    try {
      await txApi.categorizeUncategorized(accountId);

      // Poll for completion
      categorizePollRef.current = setInterval(async () => {
        try {
          const status = await txApi.getCategorizeStatus(accountId);
          if (status.status === 'complete') {
            if (categorizePollRef.current) clearInterval(categorizePollRef.current);
            categorizePollRef.current = null;
            setCategorizing(false);

            if (status.total > 0) {
              setCategorizationSummary({
                total: status.total,
                categories: status.categories,
              });
              // Auto-dismiss after 15 seconds
              setTimeout(() => setCategorizationSummary(null), 15000);
            }

            // Refresh the transaction list
            void fetchTransactions();
          } else if (status.status === 'failed') {
            if (categorizePollRef.current) clearInterval(categorizePollRef.current);
            categorizePollRef.current = null;
            setCategorizing(false);
            console.error('Categorization failed:', status.error);
          }
        } catch {
          if (categorizePollRef.current) clearInterval(categorizePollRef.current);
          categorizePollRef.current = null;
          setCategorizing(false);
        }
      }, 2000);
    } catch (err) {
      setCategorizing(false);
      console.error('Failed to start categorization:', err);
    }
  };

  const handleExportCsv = () => {
    const headers = ['Date', 'Description', 'Category', 'Amount', 'Account'];
    const rows = transactions.map((t) => [
      t.date,
      t.description,
      t.category ?? '',
      String(t.amount),
      accounts.find((a) => a.id === t.account_id)?.name ?? '',
    ]);
    const csv = [headers, ...rows].map((r) => r.map((c) => `"${c}"`).join(',')).join('\n');
    const blob = new Blob([csv], { type: 'text/csv' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = 'transactions.csv';
    a.click();
    URL.revokeObjectURL(url);
  };

  return (
    <div className="p-4 md:p-6">
      <h1 className="text-xl md:text-2xl font-bold text-[var(--color-text)] mb-4 md:mb-6">
        Transactions
      </h1>

      {/* Toolbar: search + categorize */}
      <div className="mb-4 flex flex-wrap gap-2 items-center">
        <input
          type="text"
          value={searchQuery}
          onChange={(e) => {
            setSearchQuery(e.target.value);
            if (!e.target.value.trim()) setSearchResults(null);
          }}
          onKeyDown={(e) => {
            if (e.key === 'Enter') void handleSearch();
          }}
          placeholder="Quick search transactions..."
          className="flex-1 max-w-md px-3 py-2 text-sm border border-[var(--color-input-border)] bg-[var(--color-input-bg)] text-[var(--color-text)] rounded-lg focus:border-[var(--color-primary)] focus:ring-1 focus:ring-[var(--color-primary)]"
        />
        <button
          onClick={() => void handleSearch()}
          disabled={searching}
          className="px-4 py-2 text-sm bg-[var(--color-primary)] text-white rounded-lg hover:opacity-90 disabled:opacity-50 transition-colors"
        >
          {searching ? 'Searching...' : 'Search'}
        </button>
        {searchResults !== null && (
          <button
            onClick={() => {
              setSearchQuery('');
              setSearchResults(null);
            }}
            className="px-3 py-2 text-sm border border-[var(--color-border)] text-[var(--color-text-secondary)] rounded-lg hover:bg-[var(--color-surface)] transition-colors"
          >
            Clear
          </button>
        )}

        <div className="ml-auto">
          <button
            onClick={() => void handleCategorize()}
            disabled={categorizing || accounts.length === 0}
            title="Run AI categorization on all uncategorized transactions in this account"
            className="px-4 py-2 text-sm border border-[var(--color-primary)] text-[var(--color-primary)] rounded-lg hover:bg-[var(--color-primary)] hover:text-white disabled:opacity-50 transition-colors"
          >
            {categorizing ? (
              <span className="flex items-center gap-2">
                <span className="w-3 h-3 border-2 border-current border-t-transparent rounded-full animate-spin" />
                Categorizing...
              </span>
            ) : (
              'Categorize'
            )}
          </button>
        </div>
      </div>

      {/* Categorization summary banner */}
      {categorizationSummary && (
        <div
          className="mb-4 p-3 rounded-lg border border-[var(--color-primary)] bg-[var(--color-primary)]/10"
          role="status"
          aria-live="polite"
        >
          <div className="flex items-start justify-between">
            <div>
              <p className="text-sm font-medium text-[var(--color-primary)]">
                Categorized {categorizationSummary.total} transaction
                {categorizationSummary.total !== 1 ? 's' : ''}
              </p>
              <p className="text-xs text-[var(--color-text-secondary)] mt-1">
                {categorizationSummary.categories
                  .map((c) => `${categoryLabel(c.category, categoryMap)} (${c.count})`)
                  .join(', ')}
              </p>
            </div>
            <button
              onClick={() => setCategorizationSummary(null)}
              className="text-[var(--color-text-secondary)] hover:text-[var(--color-text)] text-sm ml-4"
              aria-label="Dismiss categorization summary"
            >
              &times;
            </button>
          </div>
        </div>
      )}

      {searchResults !== null && (
        <div className="mb-4 text-sm text-[var(--color-text-secondary)]">
          Showing {searchResults.length} search result{searchResults.length !== 1 ? 's' : ''} for "
          {searchQuery}"
        </div>
      )}

      {loading && transactions.length === 0 ? (
        <div className="text-center py-12 text-[var(--color-text-secondary)]">
          Loading transactions...
        </div>
      ) : (
        <TransactionTable
          transactions={searchResults ?? transactions}
          total={searchResults ? searchResults.length : total}
          page={searchResults ? 1 : page}
          perPage={searchResults ? searchResults.length : PER_PAGE}
          sorting={sorting}
          filters={filters}
          accounts={accounts}
          categories={categories}
          categoryMap={categoryMap}
          onSortingChange={setSorting}
          onPageChange={setPage}
          onFiltersChange={(f) => {
            // Deep-linkable filter keys are persisted to the URL (shareable /
            // back-button friendly). The rest live in local state.
            setSearchParams((sp) => {
              const updated = new URLSearchParams(sp);
              for (const key of ['category', 'date_from', 'date_to', 'account_id'] as const) {
                const val = f[key];
                if (val) updated.set(key, val);
                else updated.delete(key);
              }
              return updated;
            });
            const rest: TransactionFilters = {};
            if (f.search) rest.search = f.search;
            if (f.amount_min != null) rest.amount_min = f.amount_min;
            if (f.amount_max != null) rest.amount_max = f.amount_max;
            setUserFilters(rest);
            setPage(1);
          }}
          onCategoryChange={handleCategoryChange}
          onBulkCategoryChange={handleBulkCategoryChange}
          onExportCsv={handleExportCsv}
        />
      )}
    </div>
  );
}
