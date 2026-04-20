import { useState, useEffect, useCallback, useMemo } from 'react';
import { useNavigate } from 'react-router-dom';
import { useApi } from '@/hooks/useApi';
import { useConfigStore } from '@/stores/configStore';
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

// ── Sparkline (lightweight inline SVG) ───────────────────────────────
function Sparkline({
  points,
  stroke = 'currentColor',
  fill,
  className = '',
}: {
  points: number[];
  stroke?: string;
  fill?: string;
  className?: string;
}) {
  if (points.length < 2) return <div className={className} aria-hidden="true" />;
  const w = 100;
  const h = 32;
  const min = Math.min(...points);
  const max = Math.max(...points);
  const range = max - min || 1;
  const step = w / (points.length - 1);
  const coords = points.map((v, i) => {
    const x = i * step;
    const y = h - ((v - min) / range) * h;
    return [x, y] as const;
  });
  const path = coords
    .map(([x, y], i) => `${i === 0 ? 'M' : 'L'}${x.toFixed(2)},${y.toFixed(2)}`)
    .join(' ');
  const area = `${path} L${w},${h} L0,${h} Z`;
  return (
    <svg
      viewBox={`0 0 ${w} ${h}`}
      preserveAspectRatio="none"
      className={className}
      aria-hidden="true"
    >
      {fill && <path d={area} fill={fill} />}
      <path
        d={path}
        fill="none"
        stroke={stroke}
        strokeWidth={1.5}
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}

// ── Delta pill ───────────────────────────────────────────────────────
function DeltaPill({ delta, suffix = '' }: { delta: number | null; suffix?: string }) {
  if (delta === null || !isFinite(delta)) return null;
  const up = delta > 0;
  const flat = delta === 0;
  const cls = flat
    ? 'bg-[var(--color-border)]/50 text-[var(--color-text-secondary)]'
    : up
      ? 'bg-emerald-500/10 text-emerald-500 dark:text-emerald-400'
      : 'bg-rose-500/10 text-rose-500 dark:text-rose-400';
  const arrow = flat ? '→' : up ? '↑' : '↓';
  return (
    <span
      className={`num inline-flex items-center gap-0.5 rounded-full px-1.5 py-0.5 text-[10px] font-semibold tracking-tight ${cls}`}
    >
      <span aria-hidden="true">{arrow}</span>
      {Math.abs(delta).toFixed(1)}
      {suffix}
    </span>
  );
}

// ── Unified widget card (header + toolbar + body slots) ─────────────
function WidgetCard({
  title,
  tooltip,
  toolbar,
  headerAccent,
  children,
}: {
  title: string;
  tooltip?: string;
  toolbar?: React.ReactNode;
  headerAccent?: string;
  children: React.ReactNode;
}) {
  return (
    <div className="group relative flex h-full min-h-0 flex-col overflow-hidden rounded-2xl border border-[var(--color-border)] bg-[var(--color-card)] shadow-[var(--card-shadow)] backdrop-blur-sm transition-shadow duration-300 hover:shadow-[var(--card-shadow-hover)]">
      {headerAccent && (
        <span
          aria-hidden="true"
          className="pointer-events-none absolute inset-x-0 top-0 h-px opacity-70"
          style={{
            background: `linear-gradient(90deg, transparent, ${headerAccent}, transparent)`,
          }}
        />
      )}
      <div className="flex shrink-0 items-center justify-between gap-3 px-5 pt-4 pb-3">
        <h3
          className={`text-[11px] font-semibold uppercase tracking-[0.15em] text-[var(--color-text-secondary)] ${
            tooltip ? 'cursor-help' : ''
          }`}
          title={tooltip}
        >
          {title}
          {tooltip && (
            <span aria-hidden="true" className="ml-1 align-baseline opacity-60">
              ⓘ
            </span>
          )}
        </h3>
        {toolbar && <div className="flex items-center gap-1">{toolbar}</div>}
      </div>
      <div className="min-h-0 flex-1 px-5 pb-5">{children}</div>
    </div>
  );
}

// ── Hero KPI card (value + delta + sparkline) ───────────────────────
function KpiCard({
  label,
  value,
  delta,
  deltaSuffix,
  accent,
  sparkPoints,
  icon,
}: {
  label: string;
  value: string;
  delta?: number | null;
  deltaSuffix?: string;
  accent: string;
  sparkPoints?: number[];
  icon?: React.ReactNode;
}) {
  return (
    <div className="group relative flex flex-col overflow-hidden rounded-2xl border border-[var(--color-border)] bg-[var(--color-card)] p-5 shadow-[var(--card-shadow)] backdrop-blur-sm transition-shadow duration-300 hover:shadow-[var(--card-shadow-hover)]">
      <span
        aria-hidden="true"
        className="pointer-events-none absolute inset-x-0 top-0 h-[2px] opacity-80"
        style={{ background: `linear-gradient(90deg, transparent, ${accent}, transparent)` }}
      />
      <div className="flex items-center justify-between">
        <span className="text-[11px] font-semibold uppercase tracking-[0.15em] text-[var(--color-text-secondary)]">
          {label}
        </span>
        {icon && (
          <span
            className="flex h-7 w-7 items-center justify-center rounded-lg"
            style={{ backgroundColor: `${accent}15`, color: accent }}
            aria-hidden="true"
          >
            {icon}
          </span>
        )}
      </div>
      <div className="mt-3 flex items-baseline gap-2">
        <span className="num text-3xl font-bold leading-none tracking-[-0.025em] text-[var(--color-text)]">
          {value}
        </span>
        <DeltaPill delta={delta ?? null} suffix={deltaSuffix ?? '%'} />
      </div>
      <div className="mt-3 h-8 w-full" style={{ color: accent }}>
        <Sparkline
          points={sparkPoints ?? []}
          stroke={accent}
          fill={`${accent}22`}
          className="h-full w-full"
        />
      </div>
    </div>
  );
}

function getCurrentMonth(): string {
  const now = new Date();
  return `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, '0')}`;
}

function pctDelta(curr: number, prev: number): number | null {
  if (!isFinite(curr) || !isFinite(prev) || prev === 0) return null;
  return ((curr - prev) / Math.abs(prev)) * 100;
}

// ── Small icons ──────────────────────────────────────────────────────
const Icon = {
  wallet: (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" className="h-4 w-4">
      <path
        strokeLinecap="round"
        strokeLinejoin="round"
        d="M3 8.5A2.5 2.5 0 015.5 6h13A2.5 2.5 0 0121 8.5V17a2 2 0 01-2 2H5a2 2 0 01-2-2V8.5zM16 13h2"
      />
    </svg>
  ),
  up: (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" className="h-4 w-4">
      <path strokeLinecap="round" strokeLinejoin="round" d="M3 17l6-6 4 4 8-8M15 7h6v6" />
    </svg>
  ),
  down: (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" className="h-4 w-4">
      <path strokeLinecap="round" strokeLinejoin="round" d="M3 7l6 6 4-4 8 8M15 17h6v-6" />
    </svg>
  ),
  layers: (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" className="h-4 w-4">
      <path
        strokeLinecap="round"
        strokeLinejoin="round"
        d="M12 3l9 5-9 5-9-5 9-5zm9 9l-9 5-9-5m18 4l-9 5-9-5"
      />
    </svg>
  ),
};

export function DashboardPage() {
  const api = useApi();
  const navigate = useNavigate();
  const dashboardApi = useMemo(() => createDashboardApi(api), [api]);
  const budgetApi = useMemo(() => createBudgetApi(api), [api]);
  const upcomingWindowDays = useConfigStore((s) => s.dashboard.upcomingWindowDays);

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
  const [loading, setLoading] = useState(true);

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
            api.get<RecurringGroup[]>(`/api/recurring?upcoming=true&days=${upcomingWindowDays}`),
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
  }, [dashboardApi, budgetApi, api, upcomingWindowDays]);

  useEffect(() => {
    async function loadSpending() {
      try {
        const month = spendingMode === 'month' ? getCurrentMonth() : undefined;
        const data = await dashboardApi.getSpending(month);
        setSpendingData(data);
        setSelectedSpendingCategory(null);
        setSubcategorySpending(null);
      } catch {
        // handled by empty state
      }
    }
    if (!loading) void loadSpending();
  }, [spendingMode, dashboardApi, loading]);

  const handleCategoryClick = useCallback(
    async (category: string) => {
      // "Other" in the donut is a frontend rollup of the tail of the sorted
      // category list. It has no single backend parent, so instead of asking
      // for subcategories we show the component categories that make it up.
      // If the backend itself returned a literal "Other" row (true aggregate),
      // fall back to navigating to Transactions.
      if (category === 'Other') {
        const sorted = [...spendingData].sort((a, b) => b.amount - a.amount);
        const backendHasOther = sorted.some((c) => c.category === 'Other');
        const rolledUp = backendHasOther ? [] : sorted.slice(5);
        if (rolledUp.length === 0) {
          navigate(`/transactions?category=${encodeURIComponent(category)}`);
          return;
        }
        const total = rolledUp.reduce((s, c) => s + c.amount, 0);
        setSelectedSpendingCategory('Other');
        setSubcategorySpending(
          rolledUp.map((c) => ({
            subcategory: c.category,
            amount: c.amount,
            percentage: total > 0 ? (c.amount / total) * 100 : 0,
          })),
        );
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
    [dashboardApi, navigate, spendingMode, spendingData],
  );

  const handleDismissSubcategory = useCallback(() => {
    setSelectedSpendingCategory(null);
    setSubcategorySpending(null);
  }, []);

  // ── KPI deltas derived from historical series ──────────────────────
  const kpi = useMemo(() => {
    const nw = netWorthData;
    const last = nw[nw.length - 1];
    const prev = nw[nw.length - 2];
    const netWorthDelta = last && prev ? pctDelta(last.total, prev.total) : null;
    const assetsDelta = last && prev ? pctDelta(last.assets, prev.assets) : null;
    const liabDelta = last && prev ? pctDelta(last.liabilities, prev.liabilities) : null;
    const savingsRate = summary?.savings_rate ?? null;

    return {
      netWorthDelta,
      assetsDelta,
      liabDelta,
      savingsRate: savingsRate !== null ? savingsRate * 100 : null,
      spark: {
        total: nw.map((p) => p.total),
        assets: nw.map((p) => p.assets),
        liabilities: nw.map((p) => p.liabilities),
        net: cashflowData.map((m) => m.net),
      },
    };
  }, [netWorthData, cashflowData, summary]);

  if (loading) {
    return (
      <div className="flex h-64 items-center justify-center p-6" role="status" aria-live="polite">
        <div className="flex items-center gap-3 text-[var(--color-text-secondary)]">
          <svg
            className="h-5 w-5 animate-spin text-[var(--color-primary)]"
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

  const toggleBtn = (active: boolean, label: string, onClick: () => void) => (
    <button
      type="button"
      onClick={onClick}
      className={`rounded-md px-2 py-1 text-[11px] font-medium transition-colors ${
        active
          ? 'bg-[var(--color-primary)] text-white shadow-sm'
          : 'text-[var(--color-text-secondary)] hover:bg-[var(--color-surface)] hover:text-[var(--color-text)]'
      }`}
    >
      {label}
    </button>
  );

  return (
    <div className="p-6 lg:p-8">
      <div className="mb-8">
        <div className="mb-1.5 flex items-center gap-2">
          <span className="relative flex h-1.5 w-1.5">
            <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-[var(--color-primary)] opacity-60" />
            <span className="relative inline-flex h-1.5 w-1.5 rounded-full bg-[var(--color-primary)]" />
          </span>
          <span className="text-[11px] font-semibold uppercase tracking-[0.18em] text-[var(--color-primary)]">
            Live overview
          </span>
        </div>
        <h1 className="text-2xl font-bold tracking-tight text-[var(--color-text)]">Dashboard</h1>
        <p className="mt-1 text-sm text-[var(--color-text-secondary)]">
          Your complete financial picture, updated{' '}
          <span className="font-medium text-[var(--color-text)]">
            {new Date().toLocaleDateString('en-US', {
              weekday: 'long',
              month: 'long',
              day: 'numeric',
            })}
          </span>
        </p>
      </div>

      {/* ── Hero KPI row ─────────────────────────────────────────────── */}
      {summary && (
        <div className="mb-6 grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-4">
          <KpiCard
            label="Net Worth"
            value={formatCurrency(summary.net_worth)}
            delta={kpi.netWorthDelta}
            accent="#10b981"
            sparkPoints={kpi.spark.total}
            icon={Icon.wallet}
          />
          <KpiCard
            label="Total Assets"
            value={formatCurrency(summary.total_assets)}
            delta={kpi.assetsDelta}
            accent="#3B82F6"
            sparkPoints={kpi.spark.assets}
            icon={Icon.up}
          />
          <KpiCard
            label="Total Liabilities"
            value={formatCurrency(summary.total_liabilities)}
            delta={kpi.liabDelta !== null ? -kpi.liabDelta : null}
            accent="#F59E0B"
            sparkPoints={kpi.spark.liabilities}
            icon={Icon.down}
          />
          <KpiCard
            label="Savings Rate"
            value={kpi.savingsRate !== null ? `${kpi.savingsRate.toFixed(0)}%` : '—'}
            delta={null}
            accent="#8B5CF6"
            sparkPoints={kpi.spark.net}
            icon={Icon.layers}
          />
        </div>
      )}

      {/* ── Row 2: Net Worth + Health Score ──────────────────────────── */}
      <div className="mb-4 grid grid-cols-1 gap-4 lg:grid-cols-4 lg:auto-rows-[340px]">
        <div className="lg:col-span-3">
          <WidgetCard
            title="Net Worth"
            headerAccent="#10b981"
            toolbar={
              <span className="text-[11px] text-[var(--color-text-secondary)]">Last 12 months</span>
            }
          >
            {netWorthData.length > 0 ? (
              <div className="h-full w-full">
                <NetWorthChart data={netWorthData} />
              </div>
            ) : (
              <EmptyState message="No net worth data yet" />
            )}
          </WidgetCard>
        </div>

        <div className="lg:col-span-1">
          <WidgetCard title="Financial Health" headerAccent="#3B82F6">
            {healthData ? (
              <HealthScoreGauge data={healthData} />
            ) : (
              <EmptyState message="No health score data yet" />
            )}
          </WidgetCard>
        </div>
      </div>

      {/* ── Row 3: Cash Flow + Spending Donut ────────────────────────── */}
      <div className="mb-4 grid grid-cols-1 gap-4 lg:grid-cols-2 lg:auto-rows-[380px]">
        <WidgetCard
          title="Cash Flow"
          headerAccent="#22C55E"
          toolbar={
            <span className="text-[11px] text-[var(--color-text-secondary)]">
              Income vs. Expenses · Net
            </span>
          }
        >
          {cashflowData.length > 0 ? (
            <div className="h-full w-full">
              <CashFlowChart data={cashflowData} />
            </div>
          ) : (
            <EmptyState message="No cash flow data yet" />
          )}
        </WidgetCard>

        <WidgetCard
          title="Spending by Category"
          headerAccent="#8B5CF6"
          toolbar={
            <div className="flex gap-0.5 rounded-lg bg-[var(--color-surface)] p-0.5">
              {toggleBtn(spendingMode === 'month', 'Month', () => setSpendingMode('month'))}
              {toggleBtn(spendingMode === 'all', 'All Time', () => setSpendingMode('all'))}
            </div>
          }
        >
          {spendingData.length > 0 ? (
            <SpendingDonut
              data={spendingData}
              onCategoryClick={handleCategoryClick}
              selectedCategory={selectedSpendingCategory}
              subcategoryData={subcategorySpending}
              onDismissSubcategory={handleDismissSubcategory}
            />
          ) : (
            <EmptyState
              message={
                spendingMode === 'month'
                  ? 'No spending data for this month'
                  : 'No spending data yet'
              }
            />
          )}
        </WidgetCard>
      </div>

      {/* ── Row 4: Upcoming + Budget ─────────────────────────────────── */}
      <div className="grid grid-cols-1 gap-4 lg:grid-cols-2 lg:auto-rows-[minmax(320px,auto)]">
        <WidgetCard
          title="Upcoming"
          tooltip={`Showing recurring items with a next-expected date within the next ${upcomingWindowDays} day${upcomingWindowDays === 1 ? '' : 's'}.`}
          headerAccent="#F59E0B"
          toolbar={
            <div className="flex gap-0.5 rounded-lg bg-[var(--color-surface)] p-0.5">
              {toggleBtn(billsMode === 'expenses', 'Expenses', () => setBillsMode('expenses'))}
              {toggleBtn(billsMode === 'income', 'Income', () => setBillsMode('income'))}
            </div>
          }
        >
          <UpcomingList bills={upcomingBills} mode={billsMode} windowDays={upcomingWindowDays} />
        </WidgetCard>

        <WidgetCard
          title="Budget vs Actual"
          headerAccent="#EC4899"
          toolbar={
            <a
              href="/budget"
              className="text-[11px] font-medium text-[var(--color-primary)] hover:underline"
            >
              Manage →
            </a>
          }
        >
          {budgetData.length > 0 ? (
            <div className="flex h-full flex-col gap-3 overflow-y-auto pr-1">
              {budgetData.map((b) => (
                <BudgetProgress key={b.category} data={b} />
              ))}
            </div>
          ) : (
            <EmptyState
              message="No budget yet"
              action={
                <a
                  href="/budget"
                  className="font-medium text-[var(--color-primary)] hover:underline"
                >
                  Set up a budget →
                </a>
              }
            />
          )}
        </WidgetCard>
      </div>
    </div>
  );
}

// ── Helpers ─────────────────────────────────────────────────────────
function EmptyState({ message, action }: { message: string; action?: React.ReactNode }) {
  return (
    <div className="flex h-full w-full flex-col items-center justify-center gap-2 text-center">
      <p className="text-sm text-[var(--color-text-secondary)]">{message}</p>
      {action}
    </div>
  );
}

function UpcomingList({
  bills,
  mode,
  windowDays,
}: {
  bills: RecurringGroup[];
  mode: 'expenses' | 'income';
  windowDays: number;
}) {
  const NON_INCOME_CATEGORIES = new Set(['transfer', 'debt_payment', 'investment']);
  const now = new Date();
  now.setHours(0, 0, 0, 0);
  const horizon = new Date(now);
  horizon.setDate(horizon.getDate() + windowDays);
  const withinWindow = (dateStr?: string | null) => {
    if (!dateStr) return false;
    const d = new Date(dateStr);
    return d >= now && d <= horizon;
  };
  const filtered = bills.filter(
    (b) =>
      withinWindow(b.next_expected_date) &&
      (mode === 'income'
        ? b.avg_amount > 0 && !NON_INCOME_CATEGORIES.has(b.category ?? '')
        : b.avg_amount < 0),
  );

  if (filtered.length === 0) {
    return <EmptyState message={`No upcoming ${mode}`} />;
  }

  return (
    <ul className="flex h-full flex-col gap-1.5 overflow-y-auto pr-1">
      {filtered.map((bill) => (
        <li
          key={bill.id}
          className="group flex items-center justify-between gap-3 rounded-xl border border-[var(--color-border)]/70 bg-[var(--color-surface)]/30 px-4 py-2.5 transition-all hover:border-[var(--color-primary)]/40 hover:bg-[var(--color-surface)]"
        >
          <div className="flex min-w-0 flex-col">
            <span className="truncate text-sm font-medium text-[var(--color-text)]">
              {toTitleCase(bill.merchant_name)}
            </span>
            <span className="text-[11px] text-[var(--color-text-secondary)]">
              {bill.next_expected_date
                ? new Date(bill.next_expected_date).toLocaleDateString('en-US', {
                    month: 'short',
                    day: 'numeric',
                  })
                : ''}
            </span>
          </div>
          <span
            className={`num shrink-0 text-sm font-semibold ${
              mode === 'income'
                ? 'text-emerald-500 dark:text-emerald-400'
                : 'text-[var(--color-text)]'
            }`}
          >
            {mode === 'income' ? '+' : ''}
            {formatCurrency(Math.abs(bill.avg_amount))}
          </span>
        </li>
      ))}
    </ul>
  );
}
