import { useState, useEffect, useCallback, useMemo } from 'react';
import { useParams } from 'react-router-dom';
import { type SortingState } from '@tanstack/react-table';
import { TransactionTable } from '@/components/tables/TransactionTable';
import { FileUpload } from '@/components/upload/FileUpload';
import { AccountSignCard } from '@/components/accounts/AccountSignCard';
import { useApi } from '@/hooks/useApi';
import { createAccountApi } from '@/api/accounts';
import { createTransactionApi } from '@/api/transactions';
import { formatCurrency } from '@/utils/format';
import { useCategories } from '@/hooks/useCategories';
import type {
  Account,
  Transaction,
  TransactionFilters,
  PaginatedResponse,
  SignConvention,
} from '@/types/models';
import { ACCOUNT_TYPE_LABELS, ACCOUNT_TYPE_ICONS } from '@/types/models';
import {
  LineChart,
  Line,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
} from 'recharts';

const PER_PAGE = 30;

function formatDate(dateStr: string | null): string {
  if (!dateStr) return 'Never';
  return new Date(dateStr).toLocaleDateString('en-US', {
    month: 'short',
    day: 'numeric',
    year: 'numeric',
  });
}

type ChartPoint = { label: string; balance: number };

/** Parse amount safely — backend sends Decimal as JSON string. */
function toNum(v: unknown): number {
  if (typeof v === 'number') return v;
  if (typeof v === 'string') return parseFloat(v) || 0;
  return 0;
}

function computeBalanceOverTime(transactions: Transaction[], currentBalance: number): ChartPoint[] {
  if (transactions.length === 0) return [];

  const sorted = [...transactions].sort((a, b) => a.date.localeCompare(b.date));
  const firstDate = sorted[0]!.date;
  const lastDate = sorted[sorted.length - 1]!.date;
  const startMs = new Date(firstDate).getTime();
  const endMs = new Date(lastDate).getTime();
  const spanDays = Math.max(1, (endMs - startMs) / (1000 * 60 * 60 * 24));

  const { bucketKey, formatLabel } = pickGranularity(spanDays);

  return buildChartPoints(sorted, currentBalance, bucketKey, formatLabel, 60);
}

function pickGranularity(spanDays: number) {
  let bucketKey: (d: string) => string;
  let formatLabel: (key: string) => string;

  if (spanDays <= 90) {
    bucketKey = (d) => d;
    formatLabel = (key) =>
      new Date(key + 'T00:00:00').toLocaleDateString('en-US', { month: 'short', day: 'numeric' });
  } else if (spanDays <= 365) {
    bucketKey = (d) => {
      const date = new Date(d + 'T00:00:00');
      const day = date.getDay();
      const monday = new Date(date);
      monday.setDate(date.getDate() - day + (day === 0 ? -6 : 1));
      return monday.toISOString().slice(0, 10);
    };
    formatLabel = (key) =>
      new Date(key + 'T00:00:00').toLocaleDateString('en-US', { month: 'short', day: 'numeric' });
  } else if (spanDays <= 365 * 5) {
    bucketKey = (d) => d.slice(0, 7);
    formatLabel = (key) =>
      new Date(key + '-01T00:00:00').toLocaleDateString('en-US', {
        month: 'short',
        year: '2-digit',
      });
  } else {
    // Yearly buckets for very long ranges
    bucketKey = (d) => d.slice(0, 4);
    formatLabel = (key) => key;
  }

  return { bucketKey, formatLabel };
}

function buildChartPoints(
  sortedTxns: Transaction[],
  currentBalance: number,
  bucketKey: (d: string) => string,
  formatLabel: (key: string) => string,
  maxBuckets: number,
): ChartPoint[] {
  const bucketNet = new Map<string, number>();
  for (const t of sortedTxns) {
    const key = bucketKey(t.date);
    bucketNet.set(key, (bucketNet.get(key) ?? 0) + toNum(t.amount));
  }

  const keys = Array.from(bucketNet.keys()).sort();
  if (keys.length === 0) return [];

  const displayKeys = keys.slice(-maxBuckets);

  // Work backwards through displayKeys to find balance at end of each period
  const endBalances = new Map<string, number>();
  let temp = toNum(currentBalance);
  for (let i = displayKeys.length - 1; i >= 0; i--) {
    const k = displayKeys[i]!;
    endBalances.set(k, Math.round(temp * 100) / 100);
    temp -= bucketNet.get(k) ?? 0;
  }

  return displayKeys.map((k) => ({
    label: formatLabel(k),
    balance: endBalances.get(k) ?? 0,
  }));
}

export function AccountDetailPage() {
  const { id } = useParams<{ id: string }>();
  const api = useApi();
  const accountApi = createAccountApi(api);
  const txApi = createTransactionApi(api);

  const { categories, categoryMap } = useCategories();
  const [account, setAccount] = useState<Account | null>(null);
  const [transactions, setTransactions] = useState<Transaction[]>([]);
  const [total, setTotal] = useState(0);
  const [page, setPage] = useState(1);
  const [sorting, setSorting] = useState<SortingState>([{ id: 'date', desc: true }]);
  const [filters, setFilters] = useState<TransactionFilters>({
    account_id: id,
  });
  const [showUpload, setShowUpload] = useState(false);
  const [loading, setLoading] = useState(true);
  const [chartTransactions, setChartTransactions] = useState<Transaction[]>([]);
  const [chartMode, setChartMode] = useState<'page' | 'all'>('page');

  useEffect(() => {
    if (!id) return;
    accountApi.getAccount(id).then(setAccount).catch(console.error);
    // Fetch larger set of transactions for balance chart
    txApi
      .listTransactions(
        { account_id: id },
        { page: 1, per_page: 5000 },
        { sort_by: 'date', sort_dir: 'asc' },
      )
      .then((res) => {
        const response = res as PaginatedResponse<Transaction>;
        setChartTransactions(response.data);
      })
      .catch(console.error);
  }, [id]); // eslint-disable-line react-hooks/exhaustive-deps

  const fetchTransactions = useCallback(async () => {
    if (!id) return;
    setLoading(true);
    try {
      const sortCol = sorting[0];
      const result = await txApi.listTransactions(
        { ...filters, account_id: id },
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
  }, [id, filters, page, sorting]); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    void fetchTransactions();
  }, [fetchTransactions]);

  // "Page" mode: chart for transactions on the current page
  const pageChartData = useMemo(() => {
    if (!account || transactions.length === 0) return [];

    const sorted = [...transactions].sort((a, b) => a.date.localeCompare(b.date));
    const firstDate = sorted[0]!.date;
    const lastDate = sorted[sorted.length - 1]!.date;
    const spanDays = Math.max(
      1,
      (new Date(lastDate).getTime() - new Date(firstDate).getTime()) / (1000 * 60 * 60 * 24),
    );
    const { bucketKey, formatLabel } = pickGranularity(spanDays);

    // For page mode we need to figure out the balance at the end of this page's
    // date range. We compute it by: currentBalance minus the sum of all
    // transactions AFTER this page's latest date.
    // Since chartTransactions has all txns sorted asc, sum amounts after lastDate.
    let balanceAtEnd = toNum(account.current_balance);
    for (let i = chartTransactions.length - 1; i >= 0; i--) {
      const ct = chartTransactions[i]!;
      if (ct.date <= lastDate) break;
      balanceAtEnd -= toNum(ct.amount);
    }

    return buildChartPoints(sorted, balanceAtEnd, bucketKey, formatLabel, 60);
  }, [transactions, chartTransactions, account]);

  // "All" mode: chart for entire transaction history
  const allChartData = useMemo(
    () =>
      account ? computeBalanceOverTime(chartTransactions, toNum(account.current_balance)) : [],
    [chartTransactions, account],
  );

  const chartData = chartMode === 'page' ? pageChartData : allChartData;

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

  const handleSignOverrideChange = useCallback(
    async (convention: SignConvention | null) => {
      if (!id) return;
      const result = await accountApi.setSignOverride(id, convention);
      // The endpoint returns the refreshed account plus stats on how
      // many rows were re-normalized. Update local state and refetch
      // transactions so any direction-flipped rows are reflected.
      setAccount(result.account);
      void fetchTransactions();
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [id],
  );

  const refreshAccountAndTransactions = useCallback(() => {
    if (!id) return;
    // Refresh account tiles (balance, transaction count, last import)
    accountApi.getAccount(id).then(setAccount).catch(console.error);
    // Refresh transactions list
    void fetchTransactions();
    // Refresh chart data
    txApi
      .listTransactions(
        { account_id: id },
        { page: 1, per_page: 5000 },
        { sort_by: 'date', sort_dir: 'asc' },
      )
      .then((res) => {
        const response = res as PaginatedResponse<Transaction>;
        setChartTransactions(response.data);
      })
      .catch(console.error);
  }, [id, fetchTransactions]); // eslint-disable-line react-hooks/exhaustive-deps

  const handleExportCsv = () => {
    const headers = ['Date', 'Description', 'Category', 'Amount'];
    const rows = transactions.map((t) => [
      t.date,
      t.description,
      t.category ?? '',
      String(t.amount),
    ]);
    const csv = [headers, ...rows].map((r) => r.map((c) => `"${c}"`).join(',')).join('\n');
    const blob = new Blob([csv], { type: 'text/csv' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `${account?.name ?? 'account'}-transactions.csv`;
    a.click();
    URL.revokeObjectURL(url);
  };

  if (!account && loading) {
    return (
      <div className="p-6 lg:p-8">
        <div className="text-center py-12 text-[var(--color-text-secondary)]">
          Loading account...
        </div>
      </div>
    );
  }

  if (!account) {
    return (
      <div className="p-6 lg:p-8">
        <div className="text-center py-12 text-[var(--color-text-secondary)]">
          Account not found
        </div>
      </div>
    );
  }

  return (
    <div className="p-6 lg:p-8 space-y-6">
      {/* Account summary card */}
      <div className="bg-[var(--color-card)] backdrop-blur-sm rounded-2xl border border-[var(--color-border)] p-6 shadow-[var(--card-shadow)]">
        <div className="flex items-start justify-between">
          <div>
            <div className="flex items-center gap-3">
              <span className="text-2xl">{ACCOUNT_TYPE_ICONS[account.account_type]}</span>
              <div>
                <h1 className="text-2xl font-bold text-[var(--color-text)] tracking-tight">
                  {account.name}
                </h1>
                {account.institution && (
                  <p className="text-sm text-[var(--color-text-secondary)]">
                    {account.institution}
                  </p>
                )}
              </div>
              <span className="badge-primary">{ACCOUNT_TYPE_LABELS[account.account_type]}</span>
            </div>
          </div>
          <button onClick={() => setShowUpload(!showUpload)} className="btn-primary text-sm">
            Import Transactions
          </button>
        </div>

        <div className="mt-4 grid grid-cols-3 gap-6">
          <div>
            <p className="text-xs font-medium text-[var(--color-text-secondary)] uppercase tracking-wider">
              Balance
            </p>
            <p
              className={`text-xl font-bold ${account.current_balance >= 0 ? 'text-[var(--color-text)]' : 'text-[var(--color-error)]'}`}
            >
              {formatCurrency(account.current_balance)}
              <span className="sr-only">
                {account.current_balance >= 0 ? ' (positive balance)' : ' (negative balance)'}
              </span>
            </p>
          </div>
          <div>
            <p className="text-xs font-medium text-[var(--color-text-secondary)] uppercase tracking-wider">
              Last Import
            </p>
            <p className="text-sm text-[var(--color-text)]">{formatDate(account.last_import_at)}</p>
          </div>
          <div>
            <p className="text-xs font-medium text-[var(--color-text-secondary)] uppercase tracking-wider">
              Transactions
            </p>
            <p className="text-sm text-[var(--color-text)]">
              {account.transaction_count.toLocaleString()}
            </p>
          </div>
        </div>
      </div>

      {/* Sign-convention card — lets the user flip if imports look reversed.
          See ADR-018. */}
      <AccountSignCard account={account} onChange={handleSignOverrideChange} />

      {/* Upload section */}
      {showUpload && (
        <div className="bg-[var(--color-card)] backdrop-blur-sm rounded-2xl border border-[var(--color-border)] p-6 shadow-[var(--card-shadow)]">
          <h2 className="text-lg font-semibold text-[var(--color-text)] mb-4">
            Import Transactions
          </h2>
          <FileUpload
            accountId={account.id}
            onTransactionsImported={refreshAccountAndTransactions}
            onCategorizationProgress={() => void fetchTransactions()}
            onImportComplete={() => {
              setShowUpload(false);
              refreshAccountAndTransactions();
            }}
          />
        </div>
      )}

      {/* Balance over time chart */}
      <div className="bg-[var(--color-card)] backdrop-blur-sm rounded-2xl border border-[var(--color-border)] p-6 shadow-[var(--card-shadow)]">
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-lg font-semibold text-[var(--color-text)]">Balance Over Time</h2>
          <div className="flex rounded-lg border border-[var(--color-border)] overflow-hidden text-xs">
            <button
              onClick={() => setChartMode('page')}
              className={`px-3 py-1.5 transition-colors ${
                chartMode === 'page'
                  ? 'bg-[var(--color-primary)] text-white'
                  : 'text-[var(--color-text-secondary)] hover:text-[var(--color-text)]'
              }`}
            >
              Current Page
            </button>
            <button
              onClick={() => setChartMode('all')}
              className={`px-3 py-1.5 transition-colors ${
                chartMode === 'all'
                  ? 'bg-[var(--color-primary)] text-white'
                  : 'text-[var(--color-text-secondary)] hover:text-[var(--color-text)]'
              }`}
            >
              All Time
            </button>
          </div>
        </div>
        <div
          role="img"
          aria-label={
            chartData.length > 0
              ? `Balance over time chart showing ${chartData.length} data points.`
              : 'Balance over time chart. Data will appear after importing transactions.'
          }
        >
          <ResponsiveContainer width="100%" height={200}>
            <LineChart data={chartData.length > 0 ? chartData : [{ label: '', balance: 0 }]}>
              <CartesianGrid strokeDasharray="3 3" stroke="var(--color-border)" />
              <XAxis dataKey="label" tick={{ fontSize: 12 }} stroke="var(--color-text-secondary)" />
              <YAxis tick={{ fontSize: 12 }} stroke="var(--color-text-secondary)" />
              <Tooltip
                formatter={(value) =>
                  Number(value).toLocaleString('en-US', { style: 'currency', currency: 'USD' })
                }
                contentStyle={{
                  backgroundColor: 'var(--color-surface)',
                  borderColor: 'var(--color-border)',
                  borderRadius: '12px',
                  color: 'var(--color-text)',
                }}
              />
              <Line
                type="monotone"
                dataKey="balance"
                stroke="var(--color-primary)"
                strokeWidth={2}
                dot={chartData.length <= 24}
              />
            </LineChart>
          </ResponsiveContainer>
          {chartData.length === 0 && (
            <p className="text-xs text-[var(--color-text-secondary)] text-center mt-2">
              Chart will populate with real data after importing transactions
            </p>
          )}
        </div>
      </div>

      {/* Transaction table */}
      <div className="bg-[var(--color-card)] backdrop-blur-sm rounded-2xl border border-[var(--color-border)] p-6 shadow-[var(--card-shadow)]">
        <h2 className="text-lg font-semibold text-[var(--color-text)] mb-4">Transactions</h2>
        <TransactionTable
          transactions={transactions}
          total={total}
          page={page}
          perPage={PER_PAGE}
          sorting={sorting}
          filters={filters}
          accounts={account ? [account] : []}
          categories={categories}
          categoryMap={categoryMap}
          lockedAccountId={id}
          onSortingChange={setSorting}
          onPageChange={setPage}
          onFiltersChange={(f) => {
            setFilters({ ...f, account_id: id });
            setPage(1);
          }}
          onCategoryChange={handleCategoryChange}
          onBulkCategoryChange={handleBulkCategoryChange}
          onExportCsv={handleExportCsv}
        />
      </div>
    </div>
  );
}
