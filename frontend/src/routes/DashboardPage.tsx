import { useState, useEffect, useCallback, useMemo } from 'react';
import { useNavigate } from 'react-router-dom';
import { ResponsiveGridLayout, useContainerWidth } from 'react-grid-layout';
import { useApi } from '@/hooks/useApi';
import { createDashboardApi } from '@/api/dashboard';
import { createBudgetApi } from '@/api/budgets';
import { formatCurrencyCompact as formatCurrency } from '@/utils/format';
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
    <div className="flex h-full flex-col overflow-hidden rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] p-4 shadow-sm">
      <h3 className="mb-3 text-sm font-semibold uppercase tracking-wider text-[var(--color-text-secondary)]">
        {title}
      </h3>
      <div className="min-h-0 flex-1">{children}</div>
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
  const [budgetData, setBudgetData] = useState<BudgetVsActual[]>([]);
  const [healthData, setHealthData] = useState<HealthScore | null>(null);
  const [upcomingBills, setUpcomingBills] = useState<RecurringGroup[]>([]);
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
    (category: string) => {
      navigate(`/transactions?category=${encodeURIComponent(category)}`);
    },
    [navigate],
  );

  if (loading) {
    return (
      <div className="flex h-64 items-center justify-center p-6" role="status" aria-live="polite">
        <div className="text-[var(--color-text-secondary)]">Loading dashboard...</div>
      </div>
    );
  }

  return (
    <div className="p-6" ref={containerRef}>
      <h1 className="mb-6 text-2xl font-bold text-[var(--color-text)]">Dashboard</h1>

      {mounted && (
        <ResponsiveGridLayout
          className="layout"
          width={width}
          layouts={layouts}
          breakpoints={{ lg: 1024, md: 768, sm: 0 }}
          cols={{ lg: 12, md: 12, sm: 12 }}
          rowHeight={60}
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
              {spendingData.length > 0 ? (
                <SpendingDonut data={spendingData} onCategoryClick={handleCategoryClick} />
              ) : (
                <p className="text-sm text-[var(--color-text-secondary)]">No spending data yet</p>
              )}
            </WidgetCard>
          </div>

          <div key="bills">
            <WidgetCard title="Upcoming Bills">
              <div className="widget-drag-handle cursor-grab" />
              {upcomingBills.length > 0 ? (
                <ul className="space-y-2">
                  {upcomingBills.map((bill) => (
                    <li
                      key={bill.id}
                      className="flex items-center justify-between rounded-md border border-[var(--color-border)] px-3 py-2"
                    >
                      <div>
                        <span className="text-sm font-medium text-[var(--color-text)]">
                          {bill.merchant_name}
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
                      <span className="text-sm font-medium text-[var(--color-text)]">
                        {formatCurrency(Math.abs(bill.average_amount))}
                      </span>
                    </li>
                  ))}
                </ul>
              ) : (
                <p className="text-sm text-[var(--color-text-secondary)]">No upcoming bills</p>
              )}
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
                  <a href="/budget" className="text-[var(--color-accent)] underline">
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
