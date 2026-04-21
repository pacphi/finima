import { useState, useEffect, useCallback, useMemo } from 'react';
import { useApi } from '@/hooks/useApi';
import { createFlowApi } from '@/api/flows';
import { createAccountApi } from '@/api/accounts';
import { usePortfolioStore } from '@/stores/portfolioStore';
import { formatCurrencyCompact as formatCurrency } from '@/utils/format';
import { InteractiveSankey } from '@/components/charts/InteractiveSankey';
import { WaterfallChart } from '@/components/charts/WaterfallChart';
import type {
  Account,
  AccountFlow,
  SubcategorySpend,
  SankeyData,
  OutflowRank,
  WaterfallData,
  FlowGroup,
} from '@/types/models';

type Tab = 'detected-flows' | 'sankey' | 'balance-impact' | 'flow-groups';

function formatMonthDisplay(month: string): string {
  // Parse year/month directly to avoid timezone issues with Date constructor.
  const [y, m] = month.split('-').map(Number) as [number, number];
  const names = [
    'January',
    'February',
    'March',
    'April',
    'May',
    'June',
    'July',
    'August',
    'September',
    'October',
    'November',
    'December',
  ];
  return `${names[m - 1]} ${y}`;
}

function shiftMonth(month: string, delta: number): string {
  // Parse year/month directly to avoid timezone-induced off-by-one errors.
  // new Date('2026-03-01') is UTC midnight, which in US timezones becomes
  // the previous day, causing getMonth() to return the wrong value.
  const [y, m] = month.split('-').map(Number) as [number, number];
  const totalMonths = y * 12 + (m - 1) + delta;
  const newYear = Math.floor(totalMonths / 12);
  const newMonth = (totalMonths % 12) + 1;
  return `${newYear}-${String(newMonth).padStart(2, '0')}`;
}

function getCurrentMonth(): string {
  const now = new Date();
  return `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, '0')}`;
}

function getTrendArrow(trend: string): string {
  if (trend.startsWith('+') || trend.includes('up') || trend.includes('increase')) return '^';
  if (trend.startsWith('-') || trend.includes('down') || trend.includes('decrease')) return 'v';
  return '-';
}

/** Human-readable label for the Outflow Ranking `Type` column.
 *  Converts raw account_type values (`credit_card`, `loan_mortgage`)
 *  and the synthetic `"category"` type into Title Case. */
function formatOutflowType(type: string): string {
  if (!type) return '—';
  return type
    .split(/[_\s-]+/)
    .filter(Boolean)
    .map((w) => w.charAt(0).toUpperCase() + w.slice(1).toLowerCase())
    .join(' ');
}

/** Convert a Title-Cased category label ("Food Dining") back to the
 *  backend slug ("food_dining") used in the `category` query param. */
function categorySlug(label: string): string {
  return label.trim().toLowerCase().replace(/\s+/g, '_');
}

/** Build a Transactions-page deep link that filters to this category
 *  for the given month. Used for "View" on category rows in the
 *  Outflow Ranking. */
function transactionsLinkForCategory(month: string, categoryLabel: string): string {
  const [y, m] = month.split('-').map(Number) as [number, number];
  const lastDay = new Date(y, m, 0).getDate(); // month is 1-based into Date(.., m, 0)
  const from = `${month}-01`;
  const to = `${month}-${String(lastDay).padStart(2, '0')}`;
  const params = new URLSearchParams({
    category: categorySlug(categoryLabel),
    date_from: from,
    date_to: to,
  });
  return `/transactions?${params.toString()}`;
}

// ── Detected Flows Tab ─────────────────────────────────────────────

function DetectedFlowsTab({
  month,
  flowApi,
  portfolioId,
}: {
  month: string;
  flowApi: ReturnType<typeof createFlowApi>;
  portfolioId: string | null;
}) {
  const [flows, setFlows] = useState<AccountFlow[]>([]);
  const [loading, setLoading] = useState(true);

  const loadFlows = useCallback(async () => {
    try {
      const data = await flowApi.listFlows(month, portfolioId);
      setFlows(data);
    } catch {
      // ignore
    } finally {
      setLoading(false);
    }
  }, [flowApi, month, portfolioId]);

  useEffect(() => {
    let ignore = false;
    (async () => {
      try {
        const data = await flowApi.listFlows(month, portfolioId);
        if (!ignore) setFlows(data);
      } catch {
        // ignore
      } finally {
        if (!ignore) setLoading(false);
      }
    })();
    return () => {
      ignore = true;
    };
  }, [flowApi, month, portfolioId]);

  const handleConfirm = useCallback(
    async (id: string) => {
      try {
        await flowApi.confirmFlow(id);
        await loadFlows();
      } catch {
        // ignore
      }
    },
    [flowApi, loadFlows],
  );

  const handleDismiss = useCallback(
    async (id: string) => {
      try {
        await flowApi.dismissFlow(id);
        setFlows((prev) => prev.filter((f) => f.id !== id));
      } catch {
        // ignore
      }
    },
    [flowApi],
  );

  if (loading) {
    return <p className="text-sm text-[var(--color-text-secondary)]">Loading...</p>;
  }

  return (
    <div className="space-y-4">
      {flows.length > 0 ? (
        <div className="overflow-hidden rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)]">
          <table className="w-full text-sm" aria-label={`Detected flows for ${month}`}>
            <thead>
              <tr className="border-b border-[var(--color-border)] bg-[var(--color-bg-secondary)]">
                <th className="px-4 py-3 text-left font-medium text-[var(--color-text-secondary)]">
                  Date
                </th>
                <th className="px-4 py-3 text-left font-medium text-[var(--color-text-secondary)]">
                  From
                </th>
                <th className="px-4 py-3 text-left font-medium text-[var(--color-text-secondary)]">
                  To
                </th>
                <th className="px-4 py-3 text-left font-medium text-[var(--color-text-secondary)]">
                  Description
                </th>
                <th className="px-4 py-3 text-right font-medium text-[var(--color-text-secondary)]">
                  Amount
                </th>
                <th className="px-4 py-3 text-center font-medium text-[var(--color-text-secondary)]">
                  Status
                </th>
                <th className="px-4 py-3 text-center font-medium text-[var(--color-text-secondary)]">
                  Actions
                </th>
              </tr>
            </thead>
            <tbody>
              {flows.map((flow) => (
                <tr key={flow.id} className="border-b border-[var(--color-border)]">
                  <td className="px-4 py-3 text-[var(--color-text)]">{flow.date}</td>
                  <td className="px-4 py-3 text-[var(--color-text)] font-medium">
                    {flow.source_account_name}
                  </td>
                  <td className="px-4 py-3 text-[var(--color-text)] font-medium">
                    {flow.destination_account_name}
                  </td>
                  <td className="px-4 py-3 text-[var(--color-text-secondary)] max-w-xs truncate">
                    {flow.source_description ?? '—'}
                  </td>
                  <td className="px-4 py-3 text-right text-[var(--color-text)]">
                    {formatCurrency(flow.amount)}
                  </td>
                  <td className="px-4 py-3 text-center">
                    {flow.is_confirmed ? (
                      <span className="inline-flex items-center rounded-full bg-green-500/10 px-2 py-0.5 text-xs font-medium text-green-500">
                        Confirmed
                      </span>
                    ) : (
                      <span className="inline-flex items-center rounded-full bg-amber-500/10 px-2 py-0.5 text-xs font-medium text-amber-500">
                        Pending
                      </span>
                    )}
                  </td>
                  <td className="px-4 py-3 text-center">
                    {!flow.is_confirmed && (
                      <div className="flex items-center justify-center gap-2">
                        <button
                          onClick={() => void handleConfirm(flow.id)}
                          className="text-xs text-[var(--color-accent)] hover:underline"
                        >
                          Confirm
                        </button>
                        <button
                          onClick={() => void handleDismiss(flow.id)}
                          className="text-xs text-red-500 hover:underline"
                        >
                          Dismiss
                        </button>
                      </div>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : (
        <div className="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] p-8 text-center text-sm text-[var(--color-text-secondary)]">
          No detected flows for this period. Flows are automatically detected when matching
          transactions appear across accounts.
        </div>
      )}
    </div>
  );
}

// ── Sankey Tab ──────────────────────────────────────────────────────

function SankeyTab({
  month,
  flowApi,
  api,
  portfolioId,
}: {
  month: string;
  flowApi: ReturnType<typeof createFlowApi>;
  api: ReturnType<typeof useApi>;
  portfolioId: string | null;
}) {
  const [sankeyData, setSankeyData] = useState<SankeyData>({
    nodes: [],
    links: [],
  });
  const [outflows, setOutflows] = useState<OutflowRank[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    async function load() {
      setLoading(true);
      try {
        const [sankey, ranking] = await Promise.allSettled([
          flowApi.getFullSankeyData(month, portfolioId),
          flowApi.getOutflowRanking(month, portfolioId),
        ]);
        if (sankey.status === 'fulfilled') setSankeyData(sankey.value);
        if (ranking.status === 'fulfilled') setOutflows(ranking.value);
      } catch {
        // ignore
      } finally {
        setLoading(false);
      }
    }
    void load();
  }, [month, flowApi, portfolioId]);

  const loadSubcategories = useCallback(
    async (category: string) => {
      const apiCategory = category.toLowerCase().replace(/\s+/g, '_');
      const pidQs = portfolioId ? `&portfolio_id=${encodeURIComponent(portfolioId)}` : '';
      return api.get<SubcategorySpend[]>(
        `/api/dashboard/spending/subcategories?category=${encodeURIComponent(apiCategory)}&month=${month}${pidQs}`,
      );
    },
    [api, month, portfolioId],
  );

  if (loading) {
    return <p className="text-sm text-[var(--color-text-secondary)]">Loading...</p>;
  }

  const totalOutflow = outflows.reduce((s, o) => s + o.monthly_amount, 0);

  return (
    <div className="space-y-6">
      {/* Interactive Progressive Sankey */}
      <InteractiveSankey data={sankeyData} onLoadSubcategories={loadSubcategories} />

      {/* Outflow Ranking Table */}
      <div>
        <h3 className="mb-3 text-sm font-semibold uppercase tracking-wider text-[var(--color-text-secondary)]">
          Outflow Ranking (from primary accounts)
        </h3>
        <div className="overflow-hidden rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)]">
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-[var(--color-border)] bg-[var(--color-bg-secondary)]">
                <th className="px-4 py-3 text-left font-medium text-[var(--color-text-secondary)]">
                  Account
                </th>
                <th className="px-4 py-3 text-left font-medium text-[var(--color-text-secondary)]">
                  Type
                </th>
                <th className="px-4 py-3 text-right font-medium text-[var(--color-text-secondary)]">
                  Monthly
                </th>
                <th className="px-4 py-3 text-right font-medium text-[var(--color-text-secondary)]">
                  % Income
                </th>
                <th className="px-4 py-3 text-center font-medium text-[var(--color-text-secondary)]">
                  Trend
                </th>
                <th className="px-4 py-3 text-center font-medium text-[var(--color-text-secondary)]">
                  Action
                </th>
              </tr>
            </thead>
            <tbody>
              {outflows.map((o, idx) => (
                <tr
                  key={o.account_id ?? `cat-${o.account_name}-${idx}`}
                  className="border-b border-[var(--color-border)]"
                >
                  <td className="px-4 py-3 text-[var(--color-text)]">{o.account_name}</td>
                  <td className="px-4 py-3 text-[var(--color-text-secondary)]">
                    {formatOutflowType(o.account_type)}
                  </td>
                  <td className="px-4 py-3 text-right text-[var(--color-text)]">
                    {formatCurrency(o.monthly_amount)}
                  </td>
                  <td className="px-4 py-3 text-right text-[var(--color-text)]">
                    {o.pct_income.toFixed(1)}%
                  </td>
                  <td className="px-4 py-3 text-center text-[var(--color-text-secondary)]">
                    {getTrendArrow(o.trend)} {o.trend}
                  </td>
                  <td className="px-4 py-3 text-center">
                    {o.account_id ? (
                      <a
                        href={`/accounts/${o.account_id}`}
                        className="text-xs text-[var(--color-accent)] hover:underline"
                      >
                        View
                      </a>
                    ) : (
                      <a
                        href={transactionsLinkForCategory(month, o.account_name)}
                        className="text-xs text-[var(--color-accent)] hover:underline"
                      >
                        View
                      </a>
                    )}
                  </td>
                </tr>
              ))}
              {outflows.length > 0 && (
                <tr className="bg-[var(--color-bg-secondary)] font-medium">
                  <td className="px-4 py-3 text-[var(--color-text)]">TOTAL OUTFLOWS</td>
                  <td />
                  <td className="px-4 py-3 text-right text-[var(--color-text)]">
                    {formatCurrency(totalOutflow)}
                  </td>
                  <td />
                  <td />
                  <td />
                </tr>
              )}
            </tbody>
          </table>
          {outflows.length === 0 && (
            <div className="p-8 text-center text-sm text-[var(--color-text-secondary)]">
              No outflow data available for this period. Make sure you have a primary income account
              configured.
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

// ── Balance Impact Tab ──────────────────────────────────────────────

function BalanceImpactTab({
  month,
  flowApi,
  accounts,
  portfolioId,
}: {
  month: string;
  flowApi: ReturnType<typeof createFlowApi>;
  accounts: Account[];
  portfolioId: string | null;
}) {
  const [selectedOverride, setSelectedOverride] = useState<string | null>(null);
  const [waterfallData, setWaterfallData] = useState<WaterfallData | null>(null);
  const [loading, setLoading] = useState(false);

  // Default to the primary income account (or first account) unless the user picks another.
  const defaultAccountId = useMemo(() => {
    const primary = accounts.find((a) => a.is_primary_income);
    return (primary ?? accounts[0])?.id ?? '';
  }, [accounts]);
  const selectedAccount = selectedOverride ?? defaultAccountId;

  useEffect(() => {
    if (!selectedAccount) return;
    let ignore = false;
    (async () => {
      try {
        const data = await flowApi.getBalanceImpact(month, selectedAccount, portfolioId);
        if (!ignore) setWaterfallData(data);
      } catch {
        if (!ignore) setWaterfallData(null);
      } finally {
        if (!ignore) setLoading(false);
      }
    })();
    return () => {
      ignore = true;
    };
  }, [month, selectedAccount, flowApi, portfolioId]);

  return (
    <div className="space-y-4">
      <div className="flex items-center gap-4">
        <label
          htmlFor="balance-impact-account"
          className="text-sm text-[var(--color-text-secondary)]"
        >
          Account:
        </label>
        <select
          id="balance-impact-account"
          value={selectedAccount}
          onChange={(e) => setSelectedOverride(e.target.value)}
          className="rounded-md border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2 text-sm text-[var(--color-text)]"
        >
          {accounts.map((a) => (
            <option key={a.id} value={a.id}>
              {a.name}
            </option>
          ))}
        </select>
      </div>

      <div className="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] p-4">
        {loading ? (
          <p className="text-sm text-[var(--color-text-secondary)]">Loading...</p>
        ) : waterfallData ? (
          <WaterfallChart data={waterfallData} />
        ) : (
          <p className="py-12 text-center text-sm text-[var(--color-text-secondary)]">
            No balance impact data available. Select a primary income account with transaction
            history.
          </p>
        )}
      </div>
    </div>
  );
}

// ── Flow Groups Tab ─────────────────────────────────────────────────

function FlowGroupsTab({
  flowApi,
  portfolioId,
}: {
  flowApi: ReturnType<typeof createFlowApi>;
  portfolioId: string | null;
}) {
  const [groups, setGroups] = useState<FlowGroup[]>([]);
  const [loading, setLoading] = useState(true);
  const [showCreate, setShowCreate] = useState(false);
  const [newName, setNewName] = useState('');
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editName, setEditName] = useState('');

  const loadGroups = useCallback(async () => {
    try {
      const data = await flowApi.listFlowGroups(portfolioId);
      setGroups(data);
    } catch {
      // ignore
    } finally {
      setLoading(false);
    }
  }, [flowApi, portfolioId]);

  useEffect(() => {
    let ignore = false;
    (async () => {
      try {
        const data = await flowApi.listFlowGroups(portfolioId);
        if (!ignore) setGroups(data);
      } catch {
        // ignore
      } finally {
        if (!ignore) setLoading(false);
      }
    })();
    return () => {
      ignore = true;
    };
  }, [flowApi, portfolioId]);

  const handleCreate = useCallback(async () => {
    if (!newName.trim()) return;
    try {
      await flowApi.createFlowGroup(newName.trim(), portfolioId);
      setShowCreate(false);
      setNewName('');
      await loadGroups();
    } catch {
      // ignore
    }
  }, [newName, flowApi, loadGroups, portfolioId]);

  const handleDelete = useCallback(
    async (id: string) => {
      try {
        await flowApi.deleteFlowGroup(id);
        await loadGroups();
      } catch {
        // ignore
      }
    },
    [flowApi, loadGroups],
  );

  const handleUpdate = useCallback(
    async (id: string) => {
      if (!editName.trim()) return;
      try {
        await flowApi.updateFlowGroup(id, editName.trim());
        setEditingId(null);
        await loadGroups();
      } catch {
        // ignore
      }
    },
    [editName, flowApi, loadGroups],
  );

  if (loading) {
    return <p className="text-sm text-[var(--color-text-secondary)]">Loading...</p>;
  }

  return (
    <div className="space-y-4">
      <div>
        <p className="mb-3 text-sm text-[var(--color-text-secondary)]">
          Flow groups let you label related transfers (e.g., &ldquo;Housing Costs&rdquo; for
          mortgage + property tax + insurance). Grouped flows collapse into a single band in the
          Sankey diagram.
        </p>
        <button
          onClick={() => setShowCreate(true)}
          className="rounded-md bg-[var(--color-accent)] px-4 py-2 text-sm font-medium text-white hover:opacity-90"
        >
          + Create Group
        </button>
      </div>

      {showCreate && (
        <div className="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] p-4">
          <h3 className="mb-3 text-sm font-semibold text-[var(--color-text)]">New Flow Group</h3>
          <div className="flex gap-3">
            <div className="flex-1">
              <label htmlFor="flow-group-name" className="sr-only">
                Group Name
              </label>
              <input
                id="flow-group-name"
                type="text"
                placeholder='e.g., "Housing Costs", "Debt Payments", "Savings"'
                value={newName}
                onChange={(e) => setNewName(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === 'Enter') void handleCreate();
                }}
                aria-required="true"
                className="w-full rounded-md border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2 text-sm text-[var(--color-text)]"
              />
            </div>
            <button
              onClick={() => void handleCreate()}
              className="rounded-md bg-[var(--color-accent)] px-4 py-2 text-sm font-medium text-white"
            >
              Create
            </button>
            <button
              onClick={() => {
                setShowCreate(false);
                setNewName('');
              }}
              className="rounded-md border border-[var(--color-border)] px-4 py-2 text-sm text-[var(--color-text)]"
            >
              Cancel
            </button>
          </div>
        </div>
      )}

      {groups.length > 0 ? (
        <div className="overflow-hidden rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)]">
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-[var(--color-border)] bg-[var(--color-bg-secondary)]">
                <th className="px-4 py-3 text-left font-medium text-[var(--color-text-secondary)]">
                  Group Name
                </th>
                <th className="px-4 py-3 text-left font-medium text-[var(--color-text-secondary)]">
                  Created
                </th>
                <th className="px-4 py-3 text-center font-medium text-[var(--color-text-secondary)]">
                  Actions
                </th>
              </tr>
            </thead>
            <tbody>
              {groups.map((g) => (
                <tr key={g.id} className="border-b border-[var(--color-border)]">
                  <td className="px-4 py-3 text-[var(--color-text)]">
                    {editingId === g.id ? (
                      <input
                        type="text"
                        value={editName}
                        onChange={(e) => setEditName(e.target.value)}
                        onKeyDown={(e) => {
                          if (e.key === 'Enter') void handleUpdate(g.id);
                          if (e.key === 'Escape') setEditingId(null);
                        }}
                        aria-label="Group name"
                        className="w-full rounded border border-[var(--color-border)] bg-[var(--color-bg)] px-2 py-1 text-sm"
                      />
                    ) : (
                      <span className="font-medium">{g.name}</span>
                    )}
                  </td>
                  <td className="px-4 py-3 text-[var(--color-text-secondary)]">
                    {new Date(g.created_at).toLocaleDateString()}
                  </td>
                  <td className="px-4 py-3 text-center">
                    {editingId === g.id ? (
                      <div className="flex items-center justify-center gap-2">
                        <button
                          onClick={() => void handleUpdate(g.id)}
                          className="text-xs text-[var(--color-accent)] hover:underline"
                        >
                          Save
                        </button>
                        <button
                          onClick={() => setEditingId(null)}
                          className="text-xs text-[var(--color-text-secondary)] hover:underline"
                        >
                          Cancel
                        </button>
                      </div>
                    ) : (
                      <div className="flex items-center justify-center gap-2">
                        <button
                          onClick={() => {
                            setEditingId(g.id);
                            setEditName(g.name);
                          }}
                          className="text-xs text-[var(--color-accent)] hover:underline"
                        >
                          Rename
                        </button>
                        <button
                          onClick={() => void handleDelete(g.id)}
                          className="text-xs text-red-500 hover:underline"
                        >
                          Delete
                        </button>
                      </div>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : (
        <div className="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] p-8 text-center text-sm text-[var(--color-text-secondary)]">
          No flow groups yet. Create groups to organize related account flows (e.g., &ldquo;Housing
          Costs&rdquo;, &ldquo;Debt Payments&rdquo;).
        </div>
      )}
    </div>
  );
}

// ── Main Page ───────────────────────────────────────────────────────

export function FlowsPage() {
  const api = useApi();
  const flowApi = useMemo(() => createFlowApi(api), [api]);
  const accountApi = useMemo(() => createAccountApi(api), [api]);
  const activePortfolioId = usePortfolioStore((s) => s.activePortfolioId);

  const [activeTab, setActiveTab] = useState<Tab>('sankey');
  const [month, setMonth] = useState(getCurrentMonth);
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [detecting, setDetecting] = useState(false);
  const [detectResult, setDetectResult] = useState<string | null>(null);
  // Bump this to force tab data refresh after detection.
  const [refreshKey, setRefreshKey] = useState(0);

  const handleDetectFlows = useCallback(async () => {
    setDetecting(true);
    setDetectResult(null);
    try {
      const result = await flowApi.detectFlows(month, activePortfolioId);
      setDetectResult(`Detected ${result.detected} flows, created ${result.created} new.`);
      setRefreshKey((k) => k + 1);
    } catch (err) {
      setDetectResult(err instanceof Error ? err.message : 'Detection failed.');
    } finally {
      setDetecting(false);
    }
  }, [flowApi, month, activePortfolioId]);

  // Fetch accounts for the active portfolio.
  useEffect(() => {
    if (!activePortfolioId) return;
    void accountApi
      .listAccounts(activePortfolioId)
      .then(setAccounts)
      .catch(() => {});
  }, [accountApi, activePortfolioId]);

  const tabs: { id: Tab; label: string }[] = [
    { id: 'detected-flows', label: 'Detected Flows' },
    { id: 'sankey', label: 'Sankey' },
    { id: 'balance-impact', label: 'Balance Impact' },
    { id: 'flow-groups', label: 'Flow Groups' },
  ];

  return (
    <div className="p-6">
      {/* Header with month nav */}
      <div className="mb-6 flex items-center justify-between">
        <div className="flex items-center gap-4">
          <h1 className="text-2xl font-bold text-[var(--color-text)]">Money Flow</h1>
          <button
            onClick={() => void handleDetectFlows()}
            disabled={detecting}
            className="rounded-md border border-[var(--color-border)] px-3 py-1.5 text-sm text-[var(--color-text)] hover:bg-[var(--color-bg-secondary)] disabled:opacity-50"
          >
            {detecting ? 'Detecting...' : 'Detect Flows'}
          </button>
          {detectResult && (
            <span className="text-sm text-[var(--color-text-secondary)]">{detectResult}</span>
          )}
        </div>
        <div className="flex items-center gap-3" role="group" aria-label="Month navigation">
          <button
            onClick={() => setMonth((m) => shiftMonth(m, -1))}
            aria-label="Previous month"
            className="rounded-md border border-[var(--color-border)] px-3 py-1.5 text-sm text-[var(--color-text)] hover:bg-[var(--color-bg-secondary)]"
          >
            Prev
          </button>
          <span className="text-sm font-medium text-[var(--color-text)]" aria-live="polite">
            {formatMonthDisplay(month)}
          </span>
          <button
            onClick={() => setMonth((m) => shiftMonth(m, 1))}
            aria-label="Next month"
            className="rounded-md border border-[var(--color-border)] px-3 py-1.5 text-sm text-[var(--color-text)] hover:bg-[var(--color-bg-secondary)]"
          >
            Next
          </button>
        </div>
      </div>

      {/* Tab nav */}
      <div
        className="mb-6 flex gap-1 rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-secondary)] p-1"
        role="tablist"
        aria-label="Money flow views"
      >
        {tabs.map((tab) => (
          <button
            key={tab.id}
            onClick={() => setActiveTab(tab.id)}
            role="tab"
            aria-selected={activeTab === tab.id}
            aria-controls={`tabpanel-${tab.id}`}
            id={`tab-${tab.id}`}
            className={`rounded-md px-4 py-2 text-sm font-medium transition-colors ${
              activeTab === tab.id
                ? 'bg-[var(--color-bg)] text-[var(--color-text)] shadow-sm'
                : 'text-[var(--color-text-secondary)] hover:text-[var(--color-text)]'
            }`}
          >
            {tab.label}
          </button>
        ))}
      </div>

      {/* Tab content */}
      {activeTab === 'detected-flows' && (
        <div
          key={refreshKey}
          role="tabpanel"
          id="tabpanel-detected-flows"
          aria-labelledby="tab-detected-flows"
        >
          <DetectedFlowsTab month={month} flowApi={flowApi} portfolioId={activePortfolioId} />
        </div>
      )}
      {activeTab === 'sankey' && (
        <div key={refreshKey} role="tabpanel" id="tabpanel-sankey" aria-labelledby="tab-sankey">
          <SankeyTab month={month} flowApi={flowApi} api={api} portfolioId={activePortfolioId} />
        </div>
      )}
      {activeTab === 'balance-impact' && (
        <div
          key={refreshKey}
          role="tabpanel"
          id="tabpanel-balance-impact"
          aria-labelledby="tab-balance-impact"
        >
          <BalanceImpactTab
            month={month}
            flowApi={flowApi}
            accounts={accounts}
            portfolioId={activePortfolioId}
          />
        </div>
      )}
      {activeTab === 'flow-groups' && (
        <div role="tabpanel" id="tabpanel-flow-groups" aria-labelledby="tab-flow-groups">
          <FlowGroupsTab flowApi={flowApi} portfolioId={activePortfolioId} />
        </div>
      )}
    </div>
  );
}
