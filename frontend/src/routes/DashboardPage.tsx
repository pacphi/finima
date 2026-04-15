import { useState, useEffect, useCallback, useMemo } from 'react';
import { useNavigate } from 'react-router-dom';
import { ResponsiveGridLayout, useContainerWidth } from 'react-grid-layout';
import { useApi } from '@/hooks/useApi';
import { createDashboardApi } from '@/api/dashboard';
import { createBudgetApi } from '@/api/budgets';
import { formatCurrencyCompact as formatCurrency, toTitleCase } from '@/utils/format';
import { NetWorthChart } from '@/components/charts/NetWorthChart';
import { CashFlowChart } from '@/components/charts/CashFlowChart';
import { SpendingDonut } from '@/components/charts/SpendingDonut';
import { BudgetProgress } from '@/components/charts/BudgetProgress';
import { HealthScoreGauge } from '@/components/charts/HealthScoreGauge';
import type {
  DashboardSummary,
  NetWorthPoint,
  MonthlyCashFlow,
  CategorySpend,
  SubcategorySpend,
  BudgetVsActual,
  HealthScore,
  RecurringGroup,
} from '@/types/models';
import type { ResponsiveLayouts, Layout } from 'react-grid-layout';
import 'react-grid-layout/css/styles.css';

const LAYOUT_KEY = 'finima-dashboard-layout';

const DEFAULT_LAYOUTS: ResponsiveLayouts = {
  lg: [
    { i: 'net-worth', x: 0, y: 0, w: 6, h: 4, minW: 4, minH: 3 },
    { i: 'health', x: 6, y: 0, w: 6, h: 4, minW: 4, minH: 3 },
    { i: 'cashflow', x: 0, y: 4, w: 6, h: 4, minW: 4, minH: 3 },
    { i: 'spending', x: 6, y: 4, w: 6, h: 5, minW: 4, minH: 3 },
    { i: 'bills', x: 0, y: 8, w: 6, h: 4, minW: 4, minH: 3 },
    { i: 'budget', x: 6, y: 9, w: 6, h: 5, minW: 4, minH: 3 },
  ],
  md: [
    { i: 'net-worth', x: 0, y: 0, w: 6, h: 4, minW: 4, minH: 3 },
    { i: 'health', x: 6, y: 0, w: 6, h: 4, minW: 4, minH: 3 },
    { i: 'cashflow', x: 0, y: 4, w: 6, h: 4, minW: 4, minH: 3 },
    { i: 'spending', x: 6, y: 4, w: 6, h: 5, minW: 4, minH: 3 },
    { i: 'bills', x: 0, y: 8, w: 6, h: 4, minW: 4, minH: 3 },
    { i: 'budget', x: 6, y: 9, w: 6, h: 5, minW: 4, minH: 3 },
  ],
  sm: [
    { i: 'net-worth', x: 0, y: 0, w: 12, h: 4, minW: 12, minH: 3 },
    { i: 'health', x: 0, y: 4, w: 12, h: 4, minW: 12, minH: 3 },
    { i: 'cashflow', x: 0, y: 8, w: 12, h: 4, minW: 12, minH: 3 },
    { i: 'spending', x: 0, y: 12, w: 12, h: 5, minW: 12, minH: 3 },
    { i: 'bills', x: 0, y: 17, w: 12, h: 4, minW: 12, minH: 3 },
    { i: 'budget', x: 0, y: 21, w: 12, h: 5, minW: 12, minH: 3 },
  ],
};

function loadSavedLayouts(): ResponsiveLayouts | null {
  try {
    const saved = localStorage.getItem(LAYOUT_KEY);
    if (saved) return JSON.parse(saved) as ResponsiveLayouts;
  } catch {
    // ignore
  }
  return null;
}

function WidgetCard({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="flex h-full flex-col rounded-2xl border border-[var(--color-border)] bg-[var(--color-card)] backdrop-blur-sm p-5 pb-3 shadow-[var(--card-shadow)] hover:shadow-[var(--card-shadow-hover)] transition-shadow duration-300 overflow-y-auto overflow-x-hidden">
      <h3 className="mb-3 text-xs font-semibold uppercase tracking-widest text-[var(--color-text-secondary)]">
        {title}
      </h3>
      <div className="min-h-0 flex-1">{children}</div>
    </div>
  );
}

function SummaryCard({
  label,
  value,
  icon,
}: {
  label: string;
  value: string;
  icon: React.ReactNode;
}) {
  return (
    <div className="flex items-center gap-4 rounded-2xl border border-[var(--color-border)] bg-[var(--color-card)] backdrop-blur-sm p-5 shadow-[var(--card-shadow)]">
      <div className="flex h-11 w-11 shrink-0 items-center justify-center rounded-xl bg-[var(--color-primary-subtle)] text-[var(--color-primary)]">
        {icon}
      </div>
      <div className="min-w-0">
        <p className="text-xs font-medium uppercase tracking-wider text-[var(--color-text-secondary)]">
          {label}
        </p>
        <p className="mt-0.5 text-xl font-bold text-[var(--color-text)] truncate">{value}</p>
      </div>
    </div>
  );
}

function getCurrentMonth(): string {
  const now = new Date();
  return `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, '0')}`;
}

export function DashboardPage() {
  const api = useApi();
  const navigate = useNavigate();
  const dashboardApi = useMemo(() => createDashboardApi(api), [api]);
  const budgetApi = useMemo(() => createBudgetApi(api), [api]);

  const [summary, setSummary] = useState<DashboardSummary | null>(null);
  const [netWorthData, setNetWorthData] = useState<NetWorthPoint[]>([]);
  const [cashflowData, setCashflowData] = useState<MonthlyCashFlow[]>([]);
  const [spendingData, setSpendingData] = useState<CategorySpend[]>([]);
  const [spendingMode, setSpendingMode] = useState<'month' | 'all'>('month');
  const [selectedSpendingCategory, setSelectedSpendingCategory] = useState<string | null>(null);
  const [subcategorySpending, setSubcategorySpending] = useState<SubcategorySpend[] | null>(null);
  const [budgetData, setBudgetData] = useState<BudgetVsActual[]>([]);
  const [healthData, setHealthData] = useState<HealthScore | null>(null);
  const [upcomingBills, setUpcomingBills] = useState<RecurringGroup[]>([]);
  const [billsMode, setBillsMode] = useState<'expenses' | 'income'>('expenses');
  const [layouts, setLayouts] = useState<ResponsiveLayouts>(
    () => loadSavedLayouts() ?? DEFAULT_LAYOUTS,
  );
  const [loading, setLoading] = useState(true);
  const { width, containerRef, mounted } = useContainerWidth();

  useEffect(() => {
    async function loadData() {
      setLoading(true);
      try {
        const month = getCurrentMonth();
        const [summaryRes, netWorthRes, cashflowRes, spendingRes, budgetRes, healthRes, billsRes] =
          await Promise.allSettled([
            dashboardApi.getDashboardSummary(),
            dashboardApi.getNetWorth(12),
            dashboardApi.getCashflow(12),
            dashboardApi.getSpending(month),
            budgetApi.getBudgetVsActual(month),
            dashboardApi.getHealthScore(),
            api.get<RecurringGroup[]>('/api/recurring?upcoming=true&days=30'),
          ]);

        if (summaryRes.status === 'fulfilled') setSummary(summaryRes.value);
        if (netWorthRes.status === 'fulfilled') setNetWorthData(netWorthRes.value);
        if (cashflowRes.status === 'fulfilled') setCashflowData(cashflowRes.value);
        if (spendingRes.status === 'fulfilled') setSpendingData(spendingRes.value);
        if (budgetRes.status === 'fulfilled') setBudgetData(budgetRes.value);
        if (healthRes.status === 'fulfilled') setHealthData(healthRes.value);
        if (billsRes.status === 'fulfilled') setUpcomingBills(billsRes.value);
      } catch {
        // Errors handled per-widget via empty state
      } finally {
        setLoading(false);
      }
    }

    void loadData();
  }, [dashboardApi, budgetApi, api]);

  useEffect(() => {
    async function loadSpending() {
      try {
        const month = spendingMode === 'month' ? getCurrentMonth() : undefined;
        const data = await dashboardApi.getSpending(month);
        setSpendingData(data);
        // Clear subcategory drill-down when spending mode changes
        setSelectedSpendingCategory(null);
        setSubcategorySpending(null);
      } catch {
        // handled by empty state
      }
    }
    if (!loading) void loadSpending();
  }, [spendingMode, dashboardApi, loading]);

  const handleLayoutChange = useCallback(
    (_currentLayout: Layout, allLayouts: ResponsiveLayouts) => {
      setLayouts(allLayouts);
      try {
        localStorage.setItem(LAYOUT_KEY, JSON.stringify(allLayouts));
      } catch {
        // ignore storage errors
      }
    },
    [],
  );

  const handleCategoryClick = useCallback(
    async (category: string) => {
      if (category === 'Other') {
        // "Other" is an aggregate -- navigate instead of drilling down
        navigate(`/transactions?category=${encodeURIComponent(category)}`);
        return;
      }
      setSelectedSpendingCategory(category);
      try {
        const month = spendingMode === 'month' ? getCurrentMonth() : undefined;
        const data = await dashboardApi.getSubcategorySpending(category, month);
        setSubcategorySpending(data);
      } catch {
        setSubcategorySpending(null);
      }
    },
    [dashboardApi, navigate, spendingMode],
  );

  const handleDismissSubcategory = useCallback(() => {
    setSelectedSpendingCategory(null);
    setSubcategorySpending(null);
  }, []);

  if (loading) {
    return (
      <div className="flex h-64 items-center justify-center p-6" role="status" aria-live="polite">
        <div className="flex items-center gap-3 text-[var(--color-text-secondary)]">
          <svg
            className="animate-spin h-5 w-5 text-[var(--color-primary)]"
            fill="none"
            viewBox="0 0 24 24"
          >
            <circle
              className="opacity-25"
              cx="12"
              cy="12"
              r="10"
              stroke="currentColor"
              strokeWidth="4"
            />
            <path
              className="opacity-75"
              fill="currentColor"
              d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"
            />
          </svg>
          Loading dashboard...
        </div>
      </div>
    );
  }

  return (
    <div className="p-6 lg:p-8" ref={containerRef}>
      <div className="mb-8">
        <h1 className="text-2xl font-bold text-[var(--color-text)] tracking-tight">Dashboard</h1>
        <p className="mt-1 text-sm text-[var(--color-text-secondary)]">
          Your financial overview at a glance
        </p>
      </div>

      {/* Summary cards */}
      {summary && (
        <div className="mb-8 grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
          <SummaryCard
            label="Net Worth"
            value={formatCurrency(summary.net_worth)}
            icon={
              <svg
                className="w-5 h-5"
                fill="none"
                viewBox="0 0 24 24"
                stroke="currentColor"
                strokeWidth={1.5}
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  d="M12 6v12m-3-2.818.879.659c1.171.879 3.07.879 4.242 0 1.172-.879 1.172-2.303 0-3.182C13.536 12.219 12.768 12 12 12c-.725 0-1.45-.22-2.003-.659-1.106-.879-1.106-2.303 0-3.182s2.9-.879 4.006 0l.415.33M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0Z"
                />
              </svg>
            }
          />
          <SummaryCard
            label="Total Assets"
            value={formatCurrency(summary.total_assets)}
            icon={
              <svg
                className="w-5 h-5"
                fill="none"
                viewBox="0 0 24 24"
                stroke="currentColor"
                strokeWidth={1.5}
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  d="M2.25 18 9 11.25l4.306 4.306a11.95 11.95 0 0 1 5.814-5.518l2.74-1.22m0 0-5.94-2.281m5.94 2.28-2.28 5.941"
                />
              </svg>
            }
          />
          <SummaryCard
            label="Total Liabilities"
            value={formatCurrency(summary.total_liabilities)}
            icon={
              <svg
                className="w-5 h-5"
                fill="none"
                viewBox="0 0 24 24"
                stroke="currentColor"
                strokeWidth={1.5}
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  d="M2.25 6 9 12.75l4.286-4.286a11.948 11.948 0 0 1 4.306 6.43l.776 2.898m0 0 3.182-5.511m-3.182 5.51-5.511-3.181"
                />
              </svg>
            }
          />
          <SummaryCard
            label="Accounts"
            value={String(summary.account_count ?? 0)}
            icon={
              <svg
                className="w-5 h-5"
                fill="none"
                viewBox="0 0 24 24"
                stroke="currentColor"
                strokeWidth={1.5}
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  d="M2.25 8.25h19.5M2.25 9h19.5m-16.5 5.25h6m-6 2.25h3m-3.75 3h15a2.25 2.25 0 0 0 2.25-2.25V6.75A2.25 2.25 0 0 0 19.5 4.5h-15a2.25 2.25 0 0 0-2.25 2.25v10.5A2.25 2.25 0 0 0 4.5 19.5Z"
                />
              </svg>
            }
          />
        </div>
      )}

      {mounted && (
        <ResponsiveGridLayout
          className="layout"
          width={width}
          layouts={layouts}
          breakpoints={{ lg: 1024, md: 768, sm: 0 }}
          cols={{ lg: 12, md: 12, sm: 12 }}
          rowHeight={60}
          margin={[16, 16]}
          containerPadding={[0, 0]}
          onLayoutChange={handleLayoutChange}
          dragConfig={{ handle: '.widget-drag-handle' }}
        >
          <div key="net-worth">
            <WidgetCard title="Net Worth">
              <div className="widget-drag-handle cursor-grab" />
              {summary && (
                <p className="mb-2 text-2xl font-bold text-[var(--color-text)]">
                  {formatCurrency(summary.net_worth)}
                </p>
              )}
              {netWorthData.length > 0 ? (
                <NetWorthChart data={netWorthData} />
              ) : (
                <p className="text-sm text-[var(--color-text-secondary)]">No net worth data yet</p>
              )}
            </WidgetCard>
          </div>

          <div key="health">
            <WidgetCard title="Financial Health">
              <div className="widget-drag-handle cursor-grab" />
              {healthData ? (
                <HealthScoreGauge data={healthData} />
              ) : (
                <p className="text-sm text-[var(--color-text-secondary)]">
                  No health score data yet
                </p>
              )}
            </WidgetCard>
          </div>

          <div key="cashflow">
            <WidgetCard title="Cash Flow">
              <div className="widget-drag-handle cursor-grab" />
              {cashflowData.length > 0 ? (
                <CashFlowChart data={cashflowData} />
              ) : (
                <p className="text-sm text-[var(--color-text-secondary)]">No cash flow data yet</p>
              )}
            </WidgetCard>
          </div>

          <div key="spending">
            <WidgetCard title="Spending by Category">
              <div className="widget-drag-handle cursor-grab" />
              <div className="mb-2 flex gap-1">
                <button
                  onClick={() => setSpendingMode('month')}
                  className={`rounded-md px-2 py-0.5 text-xs font-medium transition-colors ${
                    spendingMode === 'month'
                      ? 'bg-[var(--color-primary)] text-white'
                      : 'text-[var(--color-text-secondary)] hover:text-[var(--color-text)]'
                  }`}
                >
                  This Month
                </button>
                <button
                  onClick={() => setSpendingMode('all')}
                  className={`rounded-md px-2 py-0.5 text-xs font-medium transition-colors ${
                    spendingMode === 'all'
                      ? 'bg-[var(--color-primary)] text-white'
                      : 'text-[var(--color-text-secondary)] hover:text-[var(--color-text)]'
                  }`}
                >
                  All Time
                </button>
              </div>
              {spendingData.length > 0 ? (
                <SpendingDonut
                  data={spendingData}
                  onCategoryClick={handleCategoryClick}
                  selectedCategory={selectedSpendingCategory}
                  subcategoryData={subcategorySpending}
                  onDismissSubcategory={handleDismissSubcategory}
                />
              ) : (
                <p className="text-sm text-[var(--color-text-secondary)]">
                  {spendingMode === 'month'
                    ? 'No spending data for the current month'
                    : 'No spending data yet'}
                </p>
              )}
            </WidgetCard>
          </div>

          <div key="bills">
            <WidgetCard title="Upcoming">
              <div className="widget-drag-handle cursor-grab" />
              <div className="mb-2 flex gap-1">
                <button
                  onClick={() => setBillsMode('expenses')}
                  className={`rounded-md px-2 py-0.5 text-xs font-medium transition-colors ${
                    billsMode === 'expenses'
                      ? 'bg-[var(--color-primary)] text-white'
                      : 'text-[var(--color-text-secondary)] hover:text-[var(--color-text)]'
                  }`}
                >
                  Expenses
                </button>
                <button
                  onClick={() => setBillsMode('income')}
                  className={`rounded-md px-2 py-0.5 text-xs font-medium transition-colors ${
                    billsMode === 'income'
                      ? 'bg-[var(--color-primary)] text-white'
                      : 'text-[var(--color-text-secondary)] hover:text-[var(--color-text)]'
                  }`}
                >
                  Income
                </button>
              </div>
              {(() => {
                const NON_INCOME_CATEGORIES = new Set(['transfer', 'debt_payment', 'investment']);
                const filtered = upcomingBills.filter((b) =>
                  billsMode === 'income'
                    ? b.avg_amount > 0 && !NON_INCOME_CATEGORIES.has(b.category ?? '')
                    : b.avg_amount < 0,
                );
                return filtered.length > 0 ? (
                  <ul className="space-y-2">
                    {filtered.map((bill) => (
                      <li
                        key={bill.id}
                        className="flex items-center justify-between rounded-xl border border-[var(--color-border)] px-4 py-3 transition-colors hover:bg-[var(--color-surface)]"
                      >
                        <div>
                          <span className="text-sm font-medium text-[var(--color-text)]">
                            {toTitleCase(bill.merchant_name)}
                          </span>
                          <span className="ml-2 text-xs text-[var(--color-text-secondary)]">
                            {bill.next_expected_date
                              ? new Date(bill.next_expected_date).toLocaleDateString('en-US', {
                                  month: 'short',
                                  day: 'numeric',
                                })
                              : ''}
                          </span>
                        </div>
                        <span
                          className={`text-sm font-semibold ${
                            billsMode === 'income' ? 'text-green-400' : 'text-[var(--color-text)]'
                          }`}
                        >
                          {formatCurrency(Math.abs(bill.avg_amount))}
                        </span>
                      </li>
                    ))}
                  </ul>
                ) : (
                  <p className="text-sm text-[var(--color-text-secondary)]">
                    No upcoming {billsMode}
                  </p>
                );
              })()}
            </WidgetCard>
          </div>

          <div key="budget">
            <WidgetCard title="Budget vs Actual">
              <div className="widget-drag-handle cursor-grab" />
              {budgetData.length > 0 ? (
                <div className="space-y-3">
                  {budgetData.map((b) => (
                    <BudgetProgress key={b.category} data={b} />
                  ))}
                </div>
              ) : (
                <p className="text-sm text-[var(--color-text-secondary)]">
                  No budget data yet.{' '}
                  <a
                    href="/budget"
                    className="text-[var(--color-primary)] hover:underline font-medium"
                  >
                    Set up a budget
                  </a>
                </p>
              )}
            </WidgetCard>
          </div>
        </ResponsiveGridLayout>
      )}
    </div>
  );
}
