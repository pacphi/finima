import { useState, useEffect, useCallback, useMemo } from 'react';
import { useApi } from '@/hooks/useApi';
import { createFlowApi } from '@/api/flows';
import { formatCurrencyCompact as formatCurrency } from '@/utils/format';
import { SankeyDiagram } from '@/components/charts/SankeyDiagram';
import { WaterfallChart } from '@/components/charts/WaterfallChart';
import type {
  Account,
  AccountFlow,
  SankeyData,
  OutflowRank,
  WaterfallData,
  FlowGroup,
} from '@/types/models';

type Tab = 'detected-flows' | 'sankey' | 'balance-impact' | 'flow-groups';

function formatMonthDisplay(month: string): string {
  const d = new Date(month + '-01');
  return d.toLocaleDateString('en-US', { month: 'long', year: 'numeric' });
}

function shiftMonth(month: string, delta: number): string {
  const d = new Date(month + '-01');
  d.setMonth(d.getMonth() + delta);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}`;
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

// ── Detected Flows Tab ─────────────────────────────────────────────

function DetectedFlowsTab({
  month,
  flowApi,
}: {
  month: string;
  flowApi: ReturnType<typeof createFlowApi>;
}) {
  const [flows, setFlows] = useState<AccountFlow[]>([]);
  const [loading, setLoading] = useState(true);

  const loadFlows = useCallback(async () => {
    setLoading(true);
    try {
      const data = await flowApi.listFlows(month);
      setFlows(data);
    } catch {
      // ignore
    } finally {
      setLoading(false);
    }
  }, [flowApi, month]);

  useEffect(() => {
    void loadFlows();
  }, [loadFlows]);

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
                  Source
                </th>
                <th className="px-4 py-3 text-left font-medium text-[var(--color-text-secondary)]">
                  Destination
                </th>
                <th className="px-4 py-3 text-right font-medium text-[var(--color-text-secondary)]">
                  Amount
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
                  <td className="px-4 py-3 text-[var(--color-text)]">
                    {flow.source_account_id.slice(0, 8)}...
                  </td>
                  <td className="px-4 py-3 text-[var(--color-text)]">
                    {flow.destination_account_id.slice(0, 8)}...
                  </td>
                  <td className="px-4 py-3 text-right text-[var(--color-text)]">
                    {formatCurrency(flow.amount)}
                  </td>
                  <td className="px-4 py-3 text-center">
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
}: {
  month: string;
  flowApi: ReturnType<typeof createFlowApi>;
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
          flowApi.getSankeyData(month),
          flowApi.getOutflowRanking(month),
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
  }, [month, flowApi]);

  if (loading) {
    return <p className="text-sm text-[var(--color-text-secondary)]">Loading...</p>;
  }

  const totalOutflow = outflows.reduce((s, o) => s + o.monthly_amount, 0);

  return (
    <div className="space-y-6">
      {/* Sankey Diagram */}
      <div className="overflow-x-auto rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] p-4">
        <SankeyDiagram data={sankeyData} width={700} height={400} />
      </div>

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
              {outflows.map((o) => (
                <tr key={o.account_id} className="border-b border-[var(--color-border)]">
                  <td className="px-4 py-3 text-[var(--color-text)]">{o.account_name}</td>
                  <td className="px-4 py-3 text-[var(--color-text-secondary)]">{o.account_type}</td>
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
                    <a
                      href={`/accounts/${o.account_id}`}
                      className="text-xs text-[var(--color-accent)] hover:underline"
                    >
                      View
                    </a>
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

      {/* LLM Insight Card */}
      <div className="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] p-4">
        <p className="text-sm italic text-[var(--color-text-secondary)]">
          Insight: Flow trend analysis will appear here once enough transaction data is available
          across multiple months.
        </p>
      </div>
    </div>
  );
}

// ── Balance Impact Tab ──────────────────────────────────────────────

function BalanceImpactTab({
  month,
  flowApi,
  api,
}: {
  month: string;
  flowApi: ReturnType<typeof createFlowApi>;
  api: ReturnType<typeof useApi>;
}) {
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [selectedAccount, setSelectedAccount] = useState<string>('');
  const [waterfallData, setWaterfallData] = useState<WaterfallData | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    async function loadAccounts() {
      try {
        const data = await api.get<Account[]>('/api/accounts?primary_income=true');
        setAccounts(data);
        if (data.length > 0 && !selectedAccount) {
          const first = data[0];
          if (first) setSelectedAccount(first.id);
        }
      } catch {
        // ignore
      }
    }
    void loadAccounts();
  }, [api, selectedAccount]);

  useEffect(() => {
    if (!selectedAccount) return;
    async function load() {
      setLoading(true);
      try {
        const data = await flowApi.getBalanceImpact(month, selectedAccount);
        setWaterfallData(data);
      } catch {
        setWaterfallData(null);
      } finally {
        setLoading(false);
      }
    }
    void load();
  }, [month, selectedAccount, flowApi]);

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
          onChange={(e) => setSelectedAccount(e.target.value)}
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

function FlowGroupsTab({ flowApi }: { flowApi: ReturnType<typeof createFlowApi> }) {
  const [groups, setGroups] = useState<FlowGroup[]>([]);
  const [loading, setLoading] = useState(true);
  const [showCreate, setShowCreate] = useState(false);
  const [newSource, setNewSource] = useState('');
  const [newDest, setNewDest] = useState('');
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editSource, setEditSource] = useState('');
  const [editDest, setEditDest] = useState('');

  const loadGroups = useCallback(async () => {
    setLoading(true);
    try {
      const data = await flowApi.listFlowGroups();
      setGroups(data);
    } catch {
      // ignore
    } finally {
      setLoading(false);
    }
  }, [flowApi]);

  useEffect(() => {
    void loadGroups();
  }, [loadGroups]);

  const handleCreate = useCallback(async () => {
    if (!newSource.trim() || !newDest.trim()) return;
    try {
      await flowApi.createFlowGroup({
        source_account_id: newSource.trim(),
        destination_account_id: newDest.trim(),
      });
      setShowCreate(false);
      setNewSource('');
      setNewDest('');
      await loadGroups();
    } catch {
      // ignore
    }
  }, [newSource, newDest, flowApi, loadGroups]);

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
      if (!editSource.trim() || !editDest.trim()) return;
      try {
        await flowApi.updateFlowGroup(id, {
          source_account_id: editSource.trim(),
          destination_account_id: editDest.trim(),
        });
        setEditingId(null);
        await loadGroups();
      } catch {
        // ignore
      }
    },
    [editSource, editDest, flowApi, loadGroups],
  );

  if (loading) {
    return <p className="text-sm text-[var(--color-text-secondary)]">Loading...</p>;
  }

  return (
    <div className="space-y-4">
      <button
        onClick={() => setShowCreate(true)}
        className="rounded-md bg-[var(--color-accent)] px-4 py-2 text-sm font-medium text-white hover:opacity-90"
      >
        + Create Group
      </button>

      {showCreate && (
        <div className="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] p-4">
          <h3 className="mb-3 text-sm font-semibold text-[var(--color-text)]">New Flow Group</h3>
          <div className="flex gap-3">
            <div className="flex-1">
              <label htmlFor="flow-source" className="sr-only">
                Source Account ID
              </label>
              <input
                id="flow-source"
                type="text"
                placeholder="Source Account ID"
                value={newSource}
                onChange={(e) => setNewSource(e.target.value)}
                aria-required="true"
                className="w-full rounded-md border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2 text-sm text-[var(--color-text)]"
              />
            </div>
            <div className="flex-1">
              <label htmlFor="flow-dest" className="sr-only">
                Destination Account ID
              </label>
              <input
                id="flow-dest"
                type="text"
                placeholder="Destination Account ID"
                value={newDest}
                onChange={(e) => setNewDest(e.target.value)}
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
              onClick={() => setShowCreate(false)}
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
                  Source
                </th>
                <th className="px-4 py-3 text-left font-medium text-[var(--color-text-secondary)]">
                  Destination
                </th>
                <th className="px-4 py-3 text-right font-medium text-[var(--color-text-secondary)]">
                  Avg Amount
                </th>
                <th className="px-4 py-3 text-left font-medium text-[var(--color-text-secondary)]">
                  Frequency
                </th>
                <th className="px-4 py-3 text-right font-medium text-[var(--color-text-secondary)]">
                  Flows
                </th>
                <th className="px-4 py-3 text-center font-medium text-[var(--color-text-secondary)]">
                  Action
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
                        value={editSource}
                        onChange={(e) => setEditSource(e.target.value)}
                        aria-label="Source Account ID"
                        className="w-full rounded border border-[var(--color-border)] bg-[var(--color-bg)] px-2 py-1 text-sm"
                      />
                    ) : (
                      g.source_account_id
                    )}
                  </td>
                  <td className="px-4 py-3 text-[var(--color-text)]">
                    {editingId === g.id ? (
                      <input
                        type="text"
                        value={editDest}
                        onChange={(e) => setEditDest(e.target.value)}
                        aria-label="Destination Account ID"
                        className="w-full rounded border border-[var(--color-border)] bg-[var(--color-bg)] px-2 py-1 text-sm"
                      />
                    ) : (
                      g.destination_account_id
                    )}
                  </td>
                  <td className="px-4 py-3 text-right text-[var(--color-text)]">
                    {formatCurrency(g.average_amount)}
                  </td>
                  <td className="px-4 py-3 text-[var(--color-text-secondary)]">{g.frequency}</td>
                  <td className="px-4 py-3 text-right text-[var(--color-text)]">{g.flow_count}</td>
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
                            setEditSource(g.source_account_id);
                            setEditDest(g.destination_account_id);
                          }}
                          className="text-xs text-[var(--color-accent)] hover:underline"
                        >
                          Edit
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
          No flow groups yet. Create groups to organize related account flows.
        </div>
      )}
    </div>
  );
}

// ── Main Page ───────────────────────────────────────────────────────

export function FlowsPage() {
  const api = useApi();
  const flowApi = useMemo(() => createFlowApi(api), [api]);

  const [activeTab, setActiveTab] = useState<Tab>('sankey');
  const [month, setMonth] = useState(getCurrentMonth);

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
        <h1 className="text-2xl font-bold text-[var(--color-text)]">Money Flow</h1>
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
        <div role="tabpanel" id="tabpanel-detected-flows" aria-labelledby="tab-detected-flows">
          <DetectedFlowsTab month={month} flowApi={flowApi} />
        </div>
      )}
      {activeTab === 'sankey' && (
        <div role="tabpanel" id="tabpanel-sankey" aria-labelledby="tab-sankey">
          <SankeyTab month={month} flowApi={flowApi} />
        </div>
      )}
      {activeTab === 'balance-impact' && (
        <div role="tabpanel" id="tabpanel-balance-impact" aria-labelledby="tab-balance-impact">
          <BalanceImpactTab month={month} flowApi={flowApi} api={api} />
        </div>
      )}
      {activeTab === 'flow-groups' && (
        <div role="tabpanel" id="tabpanel-flow-groups" aria-labelledby="tab-flow-groups">
          <FlowGroupsTab flowApi={flowApi} />
        </div>
      )}
    </div>
  );
}
