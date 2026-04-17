import { useState, useEffect, useCallback, useMemo } from 'react';
import { useApi } from '@/hooks/useApi';
import { formatCurrency, formatDate, toTitleCase } from '@/utils/format';
import { useCategories, categoryLabel } from '@/hooks/useCategories';
import type { RecurringGroup } from '@/types/models';

type SortField =
  | 'merchant_name'
  | 'category'
  | 'frequency'
  | 'avg_amount'
  | 'next_expected_date'
  | 'type';
type SortDir = 'asc' | 'desc';
type TypeFilter = 'income' | 'expense';

export function RecurringPage() {
  const api = useApi();
  const { categoryMap } = useCategories();

  const [groups, setGroups] = useState<RecurringGroup[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [sortBy, setSortBy] = useState<SortField>('merchant_name');
  const [sortDir, setSortDir] = useState<SortDir>('asc');
  const [typeFilter, setTypeFilter] = useState<TypeFilter>('expense');

  const loadRecurring = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const data = await api.get<RecurringGroup[]>('/api/recurring');
      setGroups(data);
    } catch {
      setError('Failed to load recurring payments.');
    } finally {
      setLoading(false);
    }
  }, [api]);

  useEffect(() => {
    void loadRecurring();
  }, [loadRecurring]);

  const handleSort = (field: SortField) => {
    if (sortBy === field) {
      setSortDir((d) => (d === 'asc' ? 'desc' : 'asc'));
    } else {
      setSortBy(field);
      setSortDir(field === 'avg_amount' ? 'desc' : 'asc');
    }
  };

  const sortIndicator = (field: SortField) => {
    if (sortBy !== field) return ' \u2195';
    return sortDir === 'asc' ? ' \u2191' : ' \u2193';
  };

  const filtered = useMemo(() => {
    return groups.filter((g) => (typeFilter === 'income' ? g.avg_amount > 0 : g.avg_amount <= 0));
  }, [groups, typeFilter]);

  const sorted = useMemo(() => {
    const dir = sortDir === 'asc' ? 1 : -1;
    return [...filtered].sort((a, b) => {
      switch (sortBy) {
        case 'merchant_name':
          return dir * a.merchant_name.localeCompare(b.merchant_name);
        case 'category':
          return dir * (a.category ?? '').localeCompare(b.category ?? '');
        case 'frequency':
          return dir * a.frequency.localeCompare(b.frequency);
        case 'avg_amount':
          return dir * (Math.abs(a.avg_amount) - Math.abs(b.avg_amount));
        case 'next_expected_date':
          return dir * (a.next_expected_date ?? '').localeCompare(b.next_expected_date ?? '');
        case 'type': {
          const aType = a.avg_amount > 0 ? 1 : 0;
          const bType = b.avg_amount > 0 ? 1 : 0;
          return dir * (aType - bType);
        }
        default:
          return 0;
      }
    });
  }, [filtered, sortBy, sortDir]);

  if (loading) {
    return (
      <div className="flex h-64 items-center justify-center p-6">
        <span className="text-[var(--color-text-secondary)]">
          Loading recurring transactions...
        </span>
      </div>
    );
  }

  return (
    <div className="p-6">
      <div className="mb-6 flex items-center justify-between gap-4">
        <h1 className="text-2xl font-bold text-[var(--color-text)]">Recurring</h1>
        <label className="flex items-center gap-2 text-sm text-[var(--color-text-secondary)]">
          <span>Type</span>
          <select
            aria-label="Filter by type"
            value={typeFilter}
            onChange={(e) => setTypeFilter(e.target.value as TypeFilter)}
            className="rounded-md border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-1.5 text-sm text-[var(--color-text)] focus:outline-none focus:ring-2 focus:ring-[var(--color-accent,#10b981)]"
          >
            <option value="expense">Expense</option>
            <option value="income">Income</option>
          </select>
        </label>
      </div>

      {error && (
        <div
          className="mb-4 rounded-lg border border-red-300 bg-red-50 p-3 text-sm text-red-700"
          role="alert"
          aria-live="assertive"
        >
          {error}
        </div>
      )}

      {sorted.length > 0 ? (
        <div className="overflow-hidden rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)]">
          <table className="w-full text-sm" aria-label="Recurring transactions">
            <thead>
              <tr className="border-b border-[var(--color-border)] bg-[var(--color-bg-secondary)]">
                <th
                  scope="col"
                  onClick={() => handleSort('merchant_name')}
                  className="px-4 py-3 text-left font-medium text-[var(--color-text-secondary)] cursor-pointer select-none hover:text-[var(--color-text)] transition-colors"
                >
                  Name{sortIndicator('merchant_name')}
                </th>
                <th
                  scope="col"
                  onClick={() => handleSort('category')}
                  className="px-4 py-3 text-left font-medium text-[var(--color-text-secondary)] cursor-pointer select-none hover:text-[var(--color-text)] transition-colors"
                >
                  Category{sortIndicator('category')}
                </th>
                <th
                  scope="col"
                  onClick={() => handleSort('frequency')}
                  className="px-4 py-3 text-left font-medium text-[var(--color-text-secondary)] cursor-pointer select-none hover:text-[var(--color-text)] transition-colors"
                >
                  Frequency{sortIndicator('frequency')}
                </th>
                <th
                  scope="col"
                  onClick={() => handleSort('avg_amount')}
                  className="px-4 py-3 text-right font-medium text-[var(--color-text-secondary)] cursor-pointer select-none hover:text-[var(--color-text)] transition-colors"
                >
                  Amount{sortIndicator('avg_amount')}
                </th>
                <th
                  scope="col"
                  onClick={() => handleSort('next_expected_date')}
                  className="px-4 py-3 text-left font-medium text-[var(--color-text-secondary)] cursor-pointer select-none hover:text-[var(--color-text)] transition-colors"
                >
                  Next Expected{sortIndicator('next_expected_date')}
                </th>
                <th
                  scope="col"
                  onClick={() => handleSort('type')}
                  className="px-4 py-3 text-center font-medium text-[var(--color-text-secondary)] cursor-pointer select-none hover:text-[var(--color-text)] transition-colors"
                >
                  Type{sortIndicator('type')}
                </th>
              </tr>
            </thead>
            <tbody>
              {sorted.map((group) => (
                <tr key={group.id} className="border-b border-[var(--color-border)]">
                  <td className="px-4 py-3 text-[var(--color-text)]">
                    <div>
                      <span className="font-medium">{toTitleCase(group.merchant_name)}</span>
                      {group.is_confirmed && (
                        <span className="ml-2 inline-block rounded-full bg-green-100 px-2 py-0.5 text-xs font-medium text-green-700">
                          Confirmed
                        </span>
                      )}
                    </div>
                  </td>
                  <td className="px-4 py-3 text-[var(--color-text-secondary)]">
                    {group.category ? categoryLabel(group.category, categoryMap) : '--'}
                  </td>
                  <td className="px-4 py-3 capitalize text-[var(--color-text-secondary)]">
                    {group.frequency}
                  </td>
                  <td className="px-4 py-3 text-right text-[var(--color-text)]">
                    {formatCurrency(Math.abs(group.avg_amount))}
                  </td>
                  <td className="px-4 py-3 text-[var(--color-text-secondary)]">
                    {group.next_expected_date ? formatDate(group.next_expected_date) : '--'}
                  </td>
                  <td className="px-4 py-3 text-center">
                    <span
                      className={`inline-block rounded-full px-2 py-0.5 text-xs font-medium ${
                        group.avg_amount > 0
                          ? 'bg-green-100 text-green-700'
                          : 'bg-red-100 text-red-700'
                      }`}
                    >
                      {group.avg_amount > 0 ? 'Income' : 'Expense'}
                    </span>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : (
        <div className="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] p-12 text-center">
          <p className="text-[var(--color-text-secondary)]">
            {groups.length > 0
              ? `No recurring ${typeFilter === 'income' ? 'income' : 'expenses'} match this filter.`
              : 'No recurring transactions detected yet. Import more transactions to allow automatic detection of recurring patterns.'}
          </p>
        </div>
      )}
    </div>
  );
}
