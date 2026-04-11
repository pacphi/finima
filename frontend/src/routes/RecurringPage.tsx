import { useState, useEffect, useCallback } from 'react';
import { useApi } from '@/hooks/useApi';
import { formatCurrency, formatDate } from '@/utils/format';
import type { RecurringGroup } from '@/types/models';

export function RecurringPage() {
  const api = useApi();

  const [groups, setGroups] = useState<RecurringGroup[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

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

  if (loading) {
    return (
      <div className="flex h-64 items-center justify-center p-6">
        <span className="text-[var(--color-text-secondary)]">Loading recurring payments...</span>
      </div>
    );
  }

  return (
    <div className="p-6">
      <div className="mb-6 flex items-center justify-between">
        <h1 className="text-2xl font-bold text-[var(--color-text)]">Recurring Payments</h1>
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

      {groups.length > 0 ? (
        <div className="overflow-hidden rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)]">
          <table className="w-full text-sm" aria-label="Recurring payments">
            <thead>
              <tr className="border-b border-[var(--color-border)] bg-[var(--color-bg-secondary)]">
                <th
                  scope="col"
                  className="px-4 py-3 text-left font-medium text-[var(--color-text-secondary)]"
                >
                  Name
                </th>
                <th
                  scope="col"
                  className="px-4 py-3 text-left font-medium text-[var(--color-text-secondary)]"
                >
                  Category
                </th>
                <th
                  scope="col"
                  className="px-4 py-3 text-left font-medium text-[var(--color-text-secondary)]"
                >
                  Frequency
                </th>
                <th
                  scope="col"
                  className="px-4 py-3 text-right font-medium text-[var(--color-text-secondary)]"
                >
                  Amount
                </th>
                <th
                  scope="col"
                  className="px-4 py-3 text-left font-medium text-[var(--color-text-secondary)]"
                >
                  Next Expected
                </th>
                <th
                  scope="col"
                  className="px-4 py-3 text-center font-medium text-[var(--color-text-secondary)]"
                >
                  Type
                </th>
                <th
                  scope="col"
                  className="px-4 py-3 text-center font-medium text-[var(--color-text-secondary)]"
                >
                  Confidence
                </th>
              </tr>
            </thead>
            <tbody>
              {groups.map((group) => (
                <tr key={group.id} className="border-b border-[var(--color-border)]">
                  <td className="px-4 py-3 text-[var(--color-text)]">
                    <div>
                      <span className="font-medium">{group.merchant_name}</span>
                      {group.is_confirmed && (
                        <span className="ml-2 inline-block rounded-full bg-green-100 px-2 py-0.5 text-xs font-medium text-green-700">
                          Confirmed
                        </span>
                      )}
                    </div>
                  </td>
                  <td className="px-4 py-3 text-[var(--color-text-secondary)]">
                    {group.category ?? '--'}
                  </td>
                  <td className="px-4 py-3 capitalize text-[var(--color-text-secondary)]">
                    {group.frequency}
                  </td>
                  <td className="px-4 py-3 text-right text-[var(--color-text)]">
                    {formatCurrency(Math.abs(group.average_amount))}
                  </td>
                  <td className="px-4 py-3 text-[var(--color-text-secondary)]">
                    {group.next_expected_date ? formatDate(group.next_expected_date) : '--'}
                  </td>
                  <td className="px-4 py-3 text-center">
                    <span
                      className={`inline-block rounded-full px-2 py-0.5 text-xs font-medium ${
                        group.is_income ? 'bg-green-100 text-green-700' : 'bg-red-100 text-red-700'
                      }`}
                    >
                      {group.is_income ? 'Income' : 'Expense'}
                    </span>
                  </td>
                  <td className="px-4 py-3 text-center text-[var(--color-text-secondary)]">
                    {(group.confidence * 100).toFixed(0)}%
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : (
        <div className="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] p-12 text-center">
          <p className="text-[var(--color-text-secondary)]">
            No recurring payments detected yet. Import more transactions to allow automatic
            detection of recurring patterns.
          </p>
        </div>
      )}
    </div>
  );
}
