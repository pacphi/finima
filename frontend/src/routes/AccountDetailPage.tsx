import { useState, useEffect, useCallback, useMemo } from 'react';
import { useParams } from 'react-router-dom';
import { type SortingState } from '@tanstack/react-table';
import { TransactionTable } from '@/components/tables/TransactionTable';
import { FileUpload } from '@/components/upload/FileUpload';
import { useApi } from '@/hooks/useApi';
import { createAccountApi } from '@/api/accounts';
import { createTransactionApi } from '@/api/transactions';
import { formatCurrency } from '@/utils/format';
import type { Account, Transaction, TransactionFilters, PaginatedResponse } from '@/types/models';
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

function computeBalanceOverTime(
  transactions: Transaction[],
  currentBalance: number,
): { month: string; balance: number }[] {
  if (transactions.length === 0) return [];

  // Group transactions by month
  const monthlyNet = new Map<string, number>();
  for (const t of transactions) {
    const month = t.date.slice(0, 7); // YYYY-MM
    monthlyNet.set(month, (monthlyNet.get(month) ?? 0) + t.amount);
  }

  // Sort months
  const months = Array.from(monthlyNet.keys()).sort();
  if (months.length === 0) return [];

  // Work backwards from current balance to build historical balances
  const allMonths = months.slice(-12); // Last 12 months max
  let balance = currentBalance;
  const balances: { month: string; balance: number }[] = [];

  // Start from newest, subtract to get older balances
  for (let i = allMonths.length - 1; i >= 0; i--) {
    const m = allMonths[i]!;
    balances.unshift({
      month: new Date(m + '-01').toLocaleDateString('en-US', {
        month: 'short',
        year: '2-digit',
      }),
      balance: Math.round(balance * 100) / 100,
    });
    balance -= monthlyNet.get(m) ?? 0;
  }

  return balances;
}

export function AccountDetailPage() {
  const { id } = useParams<{ id: string }>();
  const api = useApi();
  const accountApi = createAccountApi(api);
  const txApi = createTransactionApi(api);

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

  const chartData = useMemo(
    () => (account ? computeBalanceOverTime(chartTransactions, account.current_balance) : []),
    [chartTransactions, account],
  );

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
      <div className="p-6">
        <div className="text-center py-12 text-slate-400">Loading account...</div>
      </div>
    );
  }

  if (!account) {
    return (
      <div className="p-6">
        <div className="text-center py-12 text-slate-400">Account not found</div>
      </div>
    );
  }

  return (
    <div className="p-6 space-y-6">
      {/* Account summary card */}
      <div className="bg-white rounded-lg shadow-sm border border-slate-200 p-6">
        <div className="flex items-start justify-between">
          <div>
            <div className="flex items-center gap-3">
              <span className="text-2xl">{ACCOUNT_TYPE_ICONS[account.account_type]}</span>
              <div>
                <h1 className="text-2xl font-bold text-slate-800">{account.name}</h1>
                {account.institution && (
                  <p className="text-sm text-slate-500">{account.institution}</p>
                )}
              </div>
              <span className="inline-block px-2 py-0.5 text-xs font-medium bg-slate-100 text-slate-700 rounded-full">
                {ACCOUNT_TYPE_LABELS[account.account_type]}
              </span>
            </div>
          </div>
          <button onClick={() => setShowUpload(!showUpload)} className="btn-primary text-sm">
            Import Transactions
          </button>
        </div>

        <div className="mt-4 grid grid-cols-3 gap-6">
          <div>
            <p className="text-xs font-medium text-slate-500 uppercase">Balance</p>
            <p
              className={`text-xl font-bold ${account.current_balance >= 0 ? 'text-slate-800' : 'text-red-600'}`}
            >
              {formatCurrency(account.current_balance)}
              <span className="sr-only">
                {account.current_balance >= 0 ? ' (positive balance)' : ' (negative balance)'}
              </span>
            </p>
          </div>
          <div>
            <p className="text-xs font-medium text-slate-500 uppercase">Last Import</p>
            <p className="text-sm text-slate-700">{formatDate(account.last_import_at)}</p>
          </div>
          <div>
            <p className="text-xs font-medium text-slate-500 uppercase">Transactions</p>
            <p className="text-sm text-slate-700">{account.transaction_count.toLocaleString()}</p>
          </div>
        </div>
      </div>

      {/* Upload section */}
      {showUpload && (
        <div className="bg-white rounded-lg shadow-sm border border-slate-200 p-6">
          <h2 className="text-lg font-semibold text-slate-800 mb-4">Import Transactions</h2>
          <FileUpload
            accountId={account.id}
            onImportComplete={() => {
              setShowUpload(false);
              void fetchTransactions();
            }}
          />
        </div>
      )}

      {/* Balance over time chart */}
      <div className="bg-white rounded-lg shadow-sm border border-slate-200 p-6">
        <h2 className="text-lg font-semibold text-slate-800 mb-4">Balance Over Time</h2>
        <div
          role="img"
          aria-label={
            chartData.length > 0
              ? `Balance over time chart showing ${chartData.length} months of data.`
              : 'Balance over time chart. Data will appear after importing transactions.'
          }
        >
          <ResponsiveContainer width="100%" height={200}>
            <LineChart data={chartData.length > 0 ? chartData : [{ month: '', balance: 0 }]}>
              <CartesianGrid strokeDasharray="3 3" stroke="#e2e8f0" />
              <XAxis dataKey="month" tick={{ fontSize: 12 }} stroke="#94a3b8" />
              <YAxis tick={{ fontSize: 12 }} stroke="#94a3b8" />
              <Tooltip
                formatter={(value) =>
                  Number(value).toLocaleString('en-US', { style: 'currency', currency: 'USD' })
                }
              />
              <Line
                type="monotone"
                dataKey="balance"
                stroke="var(--color-primary)"
                strokeWidth={2}
                dot={chartData.length <= 12}
              />
            </LineChart>
          </ResponsiveContainer>
          {chartData.length === 0 && (
            <p className="text-xs text-slate-400 text-center mt-2">
              Chart will populate with real data after importing transactions
            </p>
          )}
        </div>
      </div>

      {/* Transaction table */}
      <div className="bg-white rounded-lg shadow-sm border border-slate-200 p-6">
        <h2 className="text-lg font-semibold text-slate-800 mb-4">Transactions</h2>
        <TransactionTable
          transactions={transactions}
          total={total}
          page={page}
          perPage={PER_PAGE}
          sorting={sorting}
          filters={filters}
          accounts={account ? [account] : []}
          allCategories={allCategories}
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
