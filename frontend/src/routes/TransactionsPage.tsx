import { useState, useEffect, useCallback, useMemo } from 'react';
import { type SortingState } from '@tanstack/react-table';
import { TransactionTable } from '@/components/tables/TransactionTable';
import { useApi } from '@/hooks/useApi';
import { createTransactionApi } from '@/api/transactions';
import { createAccountApi } from '@/api/accounts';
import { usePortfolioStore } from '@/stores/portfolioStore';
import type { Transaction, TransactionFilters, PaginatedResponse, Account } from '@/types/models';

const PER_PAGE = 30;

export function TransactionsPage() {
  const api = useApi();
  const txApi = createTransactionApi(api);
  const accountApi = createAccountApi(api);
  const activePortfolioId = usePortfolioStore((s) => s.activePortfolioId);
  const storedAccounts = usePortfolioStore((s) => s.accounts);

  const [transactions, setTransactions] = useState<Transaction[]>([]);
  const [total, setTotal] = useState(0);
  const [page, setPage] = useState(1);
  const [sorting, setSorting] = useState<SortingState>([{ id: 'date', desc: true }]);
  const [filters, setFilters] = useState<TransactionFilters>({});
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [loading, setLoading] = useState(false);
  const [searchQuery, setSearchQuery] = useState('');
  const [searchResults, setSearchResults] = useState<Transaction[] | null>(null);
  const [searching, setSearching] = useState(false);

  useEffect(() => {
    if (storedAccounts.length > 0) {
      setAccounts(storedAccounts);
    } else if (activePortfolioId) {
      accountApi.listAccounts(activePortfolioId).then(setAccounts).catch(console.error);
    }
  }, [activePortfolioId, storedAccounts]); // eslint-disable-line react-hooks/exhaustive-deps

  const fetchTransactions = useCallback(async () => {
    setLoading(true);
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
    void fetchTransactions();
  }, [fetchTransactions]);

  const allCategories = useMemo(() => {
    const cats = new Set<string>();
    for (const t of transactions) {
      if (t.category) cats.add(t.category);
    }
    return Array.from(cats).sort();
  }, [transactions]);

  const handleCategoryChange = async (transactionId: string, category: string) => {
    try {
      await txApi.updateTransaction(transactionId, { category });
      setTransactions((prev) =>
        prev.map((t) => (t.id === transactionId ? { ...t, category, user_overridden: true } : t)),
      );
    } catch (err) {
      console.error('Failed to update category:', err);
    }
  };

  const handleBulkCategoryChange = async (ids: string[], category: string) => {
    try {
      await txApi.bulkUpdateTransactions({ ids, category });
      setTransactions((prev) =>
        prev.map((t) => (ids.includes(t.id) ? { ...t, category, user_overridden: true } : t)),
      );
    } catch (err) {
      console.error('Failed to bulk update:', err);
    }
  };

  const handleSearch = useCallback(async () => {
    if (!searchQuery.trim()) {
      setSearchResults(null);
      return;
    }
    setSearching(true);
    try {
      const results = await txApi.searchTransactions(searchQuery.trim());
      setSearchResults(results);
    } catch (err) {
      console.error('Search failed:', err);
    } finally {
      setSearching(false);
    }
  }, [searchQuery]); // eslint-disable-line react-hooks/exhaustive-deps

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

      {/* Quick search */}
      <div className="mb-4 flex gap-2">
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
      </div>

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
          allCategories={allCategories}
          onSortingChange={setSorting}
          onPageChange={setPage}
          onFiltersChange={(f) => {
            setFilters(f);
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
