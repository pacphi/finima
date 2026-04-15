import { useState, useEffect, useMemo } from 'react';
import { useApi } from '@/hooks/useApi';
import { createPayeeRulesApi, type PayeeSummary } from '@/api/payeeRules';
import { createCategoryApi, type CategoryEntry } from '@/api/categories';
import { usePortfolioStore } from '@/stores/portfolioStore';
import { categoryLabel as catLabel } from '@/hooks/useCategories';

type SortField = 'transaction_count' | 'merchant_name' | 'category';
type SortDir = 'asc' | 'desc';

/** Encode category + subcategory into a single select value. */
function encodeValue(category: string, subcategory?: string | null): string {
  if (subcategory) return `${category}::${subcategory}`;
  return category;
}

/** Decode a select value back into category + optional subcategory. */
function decodeValue(val: string): { category: string; subcategory?: string } {
  const idx = val.indexOf('::');
  if (idx >= 0) return { category: val.slice(0, idx), subcategory: val.slice(idx + 2) };
  return { category: val };
}

export function PayeeRulesPage() {
  const api = useApi();
  const payeeApi = createPayeeRulesApi(api);
  const categoryApi = createCategoryApi(api);

  const activePortfolioId = usePortfolioStore((s) => s.activePortfolioId);

  const [payees, setPayees] = useState<PayeeSummary[]>([]);
  const [categories, setCategories] = useState<CategoryEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [search, setSearch] = useState('');
  const [categoryFilter, setCategoryFilter] = useState('');
  const [sortBy, setSortBy] = useState<SortField>('merchant_name');
  const [sortDir, setSortDir] = useState<SortDir>('asc');

  // Track which row is being edited: merchant_name -> encoded "category::subcategory" value
  const [editing, setEditing] = useState<Record<string, string>>({});
  // Track rows currently applying
  const [applying, setApplying] = useState<Set<string>>(new Set());

  /** Build a flat category map for label lookups. */
  const categoryMap = useMemo(() => {
    const map: Record<string, string> = {};
    for (const c of categories) {
      map[c.key] = c.label;
      for (const sub of c.subcategories ?? []) {
        map[sub.key] = sub.label;
      }
    }
    return map;
  }, [categories]);

  /** Set of all valid encoded option values for the category select. */
  const validEncodedValues = useMemo(() => {
    const set = new Set<string>(['']);
    for (const cat of categories) {
      set.add(encodeValue(cat.key));
      for (const sub of cat.subcategories ?? []) {
        set.add(encodeValue(cat.key, sub.key));
      }
    }
    return set;
  }, [categories]);

  useEffect(() => {
    if (!activePortfolioId) {
      setLoading(false);
      return;
    }
    setLoading(true);
    Promise.all([payeeApi.listPayeeRules(activePortfolioId), categoryApi.listCategories()])
      .then(([p, c]) => {
        setPayees(p);
        // Sort categories and subcategories alphabetically
        const sorted = [...c]
          .sort((a, b) => a.label.localeCompare(b.label))
          .map((cat) => ({
            ...cat,
            subcategories: cat.subcategories
              ? [...cat.subcategories].sort((a, b) => a.label.localeCompare(b.label))
              : cat.subcategories,
          }));
        setCategories(sorted);
      })
      .catch(console.error)
      .finally(() => setLoading(false));
  }, [activePortfolioId]); // eslint-disable-line react-hooks/exhaustive-deps

  const displayLabel = (category: string | null, subcategory?: string | null) => {
    if (!category) return 'Uncategorized';
    const catLbl = catLabel(category, categoryMap);
    if (subcategory) return `${catLbl} > ${catLabel(subcategory, categoryMap)}`;
    return catLbl;
  };

  // Get unique category keys from the payees data for the filter dropdown.
  const usedCategories = useMemo(() => {
    const set = new Set<string>();
    for (const p of payees) {
      if (p.category) set.add(p.category);
    }
    return Array.from(set).sort();
  }, [payees]);

  const handleSort = (field: SortField) => {
    if (sortBy === field) {
      setSortDir((d) => (d === 'asc' ? 'desc' : 'asc'));
    } else {
      setSortBy(field);
      setSortDir(field === 'transaction_count' ? 'desc' : 'asc');
    }
  };

  const sortIndicator = (field: SortField) => {
    if (sortBy !== field) return ' \u2195';
    return sortDir === 'asc' ? ' \u2191' : ' \u2193';
  };

  const filtered = useMemo(() => {
    let result = payees;

    if (search) {
      const q = search.toLowerCase();
      result = result.filter((p) => p.merchant_name.toLowerCase().includes(q));
    }

    if (categoryFilter === '__uncategorized') {
      result = result.filter((p) => !p.category);
    } else if (categoryFilter) {
      result = result.filter((p) => p.category === categoryFilter);
    }

    const dir = sortDir === 'asc' ? 1 : -1;
    result = [...result].sort((a, b) => {
      if (sortBy === 'transaction_count') return dir * (a.transaction_count - b.transaction_count);
      if (sortBy === 'merchant_name') return dir * a.merchant_name.localeCompare(b.merchant_name);
      return dir * (a.category ?? '').localeCompare(b.category ?? '');
    });

    return result;
  }, [payees, search, categoryFilter, sortBy, sortDir]);

  const totalTransactions = useMemo(
    () => filtered.reduce((sum, p) => sum + p.transaction_count, 0),
    [filtered],
  );

  const handleCategoryChange = (merchantName: string, encodedValue: string) => {
    setEditing((prev) => ({ ...prev, [merchantName]: encodedValue }));
  };

  const handleApply = async (payee: PayeeSummary) => {
    if (!activePortfolioId) return;
    const encoded = editing[payee.merchant_name];
    if (!encoded) return;

    const { category, subcategory } = decodeValue(encoded);

    setApplying((prev) => new Set(prev).add(payee.merchant_name));
    try {
      await payeeApi.applyPayeeRule({
        portfolio_id: activePortfolioId,
        merchant_name: payee.merchant_name,
        new_category: category,
        new_subcategory: subcategory,
      });
      // Update local state
      setPayees((prev) =>
        prev.map((p) =>
          p.merchant_name === payee.merchant_name
            ? { ...p, category, subcategory: subcategory ?? null }
            : p,
        ),
      );
      setEditing((prev) => {
        const next = { ...prev };
        delete next[payee.merchant_name];
        return next;
      });
    } catch (err) {
      console.error('Failed to apply payee rule:', err);
    } finally {
      setApplying((prev) => {
        const next = new Set(prev);
        next.delete(payee.merchant_name);
        return next;
      });
    }
  };

  const handleCancel = (merchantName: string) => {
    setEditing((prev) => {
      const next = { ...prev };
      delete next[merchantName];
      return next;
    });
  };

  if (!activePortfolioId) {
    return (
      <div className="p-6 lg:p-8">
        <h1 className="text-2xl font-bold text-[var(--color-text)] tracking-tight mb-6">
          Payee Rules
        </h1>
        <p className="text-[var(--color-text-secondary)]">
          Select a portfolio to manage payee rules.
        </p>
      </div>
    );
  }

  return (
    <div className="p-6 lg:p-8">
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-2xl font-bold text-[var(--color-text)] tracking-tight">Payee Rules</h1>
      </div>

      {/* Filters row */}
      <div className="flex flex-wrap items-end gap-4 mb-6">
        <div className="flex-1 min-w-[200px]">
          <label className="block text-xs font-medium text-[var(--color-text-secondary)] mb-1 uppercase tracking-wider">
            Search Payees
          </label>
          <input
            type="text"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder="Filter by name..."
            className="input-themed"
          />
        </div>
        <div>
          <label className="block text-xs font-medium text-[var(--color-text-secondary)] mb-1 uppercase tracking-wider">
            Category
          </label>
          <select
            value={categoryFilter}
            onChange={(e) => setCategoryFilter(e.target.value)}
            className="input-themed"
          >
            <option value="">All</option>
            <option value="__uncategorized">Uncategorized</option>
            {usedCategories.map((key) => (
              <option key={key} value={key}>
                {catLabel(key, categoryMap)}
              </option>
            ))}
          </select>
        </div>
      </div>

      {loading ? (
        <div className="text-center py-12 text-[var(--color-text-secondary)]">
          Loading payee rules...
        </div>
      ) : filtered.length === 0 ? (
        <div className="text-center py-12">
          <p className="text-[var(--color-text-secondary)]">
            {payees.length === 0
              ? 'No transactions with merchant names found.'
              : 'No payees match your filters.'}
          </p>
        </div>
      ) : (
        <>
          {/* Table */}
          <div className="bg-[var(--color-card)] backdrop-blur-sm rounded-2xl border border-[var(--color-border)] overflow-hidden">
            <table className="w-full">
              <thead>
                <tr className="border-b border-[var(--color-border)]">
                  <th
                    onClick={() => handleSort('merchant_name')}
                    className="text-left px-5 py-3 text-xs font-medium text-[var(--color-text-secondary)] uppercase tracking-wider cursor-pointer select-none hover:text-[var(--color-text)] transition-colors"
                  >
                    Payee{sortIndicator('merchant_name')}
                  </th>
                  <th
                    onClick={() => handleSort('category')}
                    className="text-left px-5 py-3 text-xs font-medium text-[var(--color-text-secondary)] uppercase tracking-wider cursor-pointer select-none hover:text-[var(--color-text)] transition-colors"
                  >
                    Category{sortIndicator('category')}
                  </th>
                  <th
                    onClick={() => handleSort('transaction_count')}
                    className="text-right px-5 py-3 text-xs font-medium text-[var(--color-text-secondary)] uppercase tracking-wider cursor-pointer select-none hover:text-[var(--color-text)] transition-colors"
                  >
                    Transactions{sortIndicator('transaction_count')}
                  </th>
                  <th className="text-right px-5 py-3 text-xs font-medium text-[var(--color-text-secondary)] uppercase tracking-wider w-[140px]">
                    Action
                  </th>
                </tr>
              </thead>
              <tbody>
                {filtered.map((payee) => {
                  const isEditing = payee.merchant_name in editing;
                  const isApplying = applying.has(payee.merchant_name);
                  const currentEncoded = isEditing
                    ? editing[payee.merchant_name]
                    : encodeValue(payee.category ?? '', payee.subcategory);
                  const originalEncoded = encodeValue(payee.category ?? '', payee.subcategory);
                  const hasChanged = isEditing && editing[payee.merchant_name] !== originalEncoded;

                  return (
                    <tr
                      key={`${payee.merchant_name}-${payee.category ?? '_'}-${payee.subcategory ?? '_'}`}
                      className="border-b border-[var(--color-border)] last:border-b-0 hover:bg-[var(--color-primary-subtle)] transition-colors"
                    >
                      <td className="px-5 py-3 text-sm font-medium text-[var(--color-text)]">
                        {payee.merchant_name}
                      </td>
                      <td className="px-5 py-3">
                        <select
                          value={currentEncoded}
                          onChange={(e) =>
                            handleCategoryChange(payee.merchant_name, e.target.value)
                          }
                          disabled={isApplying}
                          className="input-themed text-sm py-1"
                        >
                          <option value="">Uncategorized</option>
                          {!isEditing &&
                            !validEncodedValues.has(originalEncoded) &&
                            originalEncoded && (
                              <option value={originalEncoded}>
                                {displayLabel(payee.category, payee.subcategory)} (unknown)
                              </option>
                            )}
                          {categories.map((cat) => (
                            <optgroup key={cat.key} label={cat.label}>
                              <option value={encodeValue(cat.key)}>{cat.label} (general)</option>
                              {(cat.subcategories ?? []).map((sub) => (
                                <option key={sub.key} value={encodeValue(cat.key, sub.key)}>
                                  {sub.label}
                                </option>
                              ))}
                            </optgroup>
                          ))}
                        </select>
                      </td>
                      <td className="px-5 py-3 text-sm text-right text-[var(--color-text-secondary)] tabular-nums">
                        {payee.transaction_count.toLocaleString()}
                      </td>
                      <td className="px-5 py-3 text-right">
                        {hasChanged ? (
                          <span className="inline-flex gap-2">
                            <button
                              onClick={() => void handleApply(payee)}
                              disabled={isApplying}
                              className="px-3 py-1 text-xs font-medium rounded-lg btn-primary disabled:opacity-50"
                            >
                              {isApplying ? 'Applying...' : 'Apply'}
                            </button>
                            <button
                              onClick={() => handleCancel(payee.merchant_name)}
                              disabled={isApplying}
                              className="px-3 py-1 text-xs text-[var(--color-text-secondary)] hover:text-[var(--color-text)] transition-colors"
                            >
                              Cancel
                            </button>
                          </span>
                        ) : (
                          <span className="text-xs text-[var(--color-text-secondary)]">
                            {displayLabel(payee.category, payee.subcategory)}
                          </span>
                        )}
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>

          {/* Summary */}
          <p className="text-xs text-[var(--color-text-secondary)] mt-4">
            Showing {filtered.length} payee{filtered.length !== 1 ? 's' : ''} across{' '}
            {totalTransactions.toLocaleString()} transactions
          </p>
        </>
      )}
    </div>
  );
}
