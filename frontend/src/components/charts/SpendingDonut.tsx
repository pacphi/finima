import { PieChart, Pie, Cell, ResponsiveContainer, Tooltip, Legend } from 'recharts';
import type { CategorySpend, SubcategorySpend } from '@/types/models';
import { toTitleCase } from '@/utils/format';

interface SpendingDonutProps {
  data: CategorySpend[];
  onCategoryClick?: (category: string) => void;
  /** When set, shows a subcategory breakdown donut beside the main chart. */
  selectedCategory?: string | null;
  subcategoryData?: SubcategorySpend[] | null;
  onDismissSubcategory?: () => void;
}

const COLORS = [
  '#3B82F6',
  '#22C55E',
  '#F59E0B',
  '#EF4444',
  '#8B5CF6',
  '#EC4899',
  '#14B8A6',
  '#F97316',
  '#6366F1',
  '#84CC16',
];

/** Lighter shades for the subcategory chart, derived from the parent's color. */
const SUB_COLORS = [
  '#93C5FD',
  '#86EFAC',
  '#FCD34D',
  '#FCA5A5',
  '#C4B5FD',
  '#F9A8D4',
  '#5EEAD4',
  '#FDBA74',
  '#A5B4FC',
  '#BEF264',
];

function formatCurrency(value: number): string {
  return new Intl.NumberFormat('en-US', {
    style: 'currency',
    currency: 'USD',
    minimumFractionDigits: 0,
    maximumFractionDigits: 0,
  }).format(value);
}

interface TooltipPayloadEntry {
  name: string;
  value: number;
  payload: { percentage: number };
}

function CustomTooltip({ active, payload }: { active?: boolean; payload?: TooltipPayloadEntry[] }) {
  if (!active || !payload?.length) return null;
  const entry = payload[0];
  if (!entry) return null;
  return (
    <div className="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] p-3 shadow-lg">
      <p className="text-sm font-medium text-[var(--color-text)]">{toTitleCase(entry.name)}</p>
      <p className="text-sm text-[var(--color-text-secondary)]">
        {formatCurrency(entry.value)} ({entry.payload.percentage.toFixed(1)}%)
      </p>
    </div>
  );
}

function buildSummary(data: CategorySpend[]): string {
  if (data.length === 0) return 'No spending data available.';
  const sorted = [...data].sort((a, b) => b.amount - a.amount);
  const total = sorted.reduce((s, c) => s + c.amount, 0);
  const top3 = sorted.slice(0, 3);
  const topDesc = top3
    .map((c) => `${c.category} at ${formatCurrency(c.amount)} (${c.percentage.toFixed(0)}%)`)
    .join(', ');
  return `Spending breakdown across ${data.length} categories totaling ${formatCurrency(total)}. Top categories: ${topDesc}.`;
}

export function SpendingDonut({
  data,
  onCategoryClick,
  selectedCategory,
  subcategoryData,
  onDismissSubcategory,
}: SpendingDonutProps) {
  // Show top categories + group the rest as "Other".
  // If data already contains an "Other" bucket (from backend aggregation),
  // show up to 8 items without re-grouping to avoid duplicate "Other" entries.
  const sorted = [...data].sort((a, b) => b.amount - a.amount);
  const hasOther = sorted.some((c) => c.category === 'Other');
  const maxSlices = hasOther ? sorted.length : 5;
  const top = sorted.slice(0, maxSlices);
  const rest = sorted.slice(maxSlices);

  const chartData =
    rest.length > 0
      ? [
          ...top,
          {
            category: 'Other',
            amount: rest.reduce((sum, c) => sum + c.amount, 0),
            percentage: rest.reduce((sum, c) => sum + c.percentage, 0),
          },
        ]
      : top;

  const summary = buildSummary(data);

  const showSubchart = selectedCategory && subcategoryData && subcategoryData.length > 0;

  // Build subcategory chart data
  const subChartData = subcategoryData
    ? [...subcategoryData].sort((a, b) => b.amount - a.amount)
    : [];

  const total = chartData.reduce((s, c) => s + c.amount, 0);

  return (
    <div className={`flex h-full w-full gap-4 ${showSubchart ? 'flex-col xl:flex-row' : ''}`}>
      {/* Main category donut + legend side-by-side */}
      <div
        className="flex h-full min-h-0 flex-1 items-center gap-3"
        role="img"
        aria-label={summary}
      >
        <div className="relative h-full min-h-[200px] flex-[1.2]">
          <ResponsiveContainer width="100%" height="100%">
            <PieChart margin={{ top: 0, right: 0, bottom: 0, left: 0 }}>
              <Pie
                data={chartData}
                cx="50%"
                cy="50%"
                innerRadius="58%"
                outerRadius="92%"
                paddingAngle={2}
                dataKey="amount"
                nameKey="category"
                onClick={(_data, index) => {
                  const entry = chartData[index];
                  if (entry && onCategoryClick) {
                    onCategoryClick(entry.category);
                  }
                }}
                style={{ cursor: onCategoryClick ? 'pointer' : 'default' }}
              >
                {chartData.map((entry, index) => (
                  <Cell
                    key={`cell-${index}`}
                    fill={COLORS[index % COLORS.length]}
                    stroke="var(--color-card)"
                    strokeWidth={2}
                    opacity={selectedCategory && entry.category !== selectedCategory ? 0.35 : 1}
                  />
                ))}
              </Pie>
              <Tooltip content={<CustomTooltip />} />
            </PieChart>
          </ResponsiveContainer>
          {/* Center label */}
          <div className="pointer-events-none absolute inset-0 flex flex-col items-center justify-center">
            <span className="text-[10px] font-medium uppercase tracking-[0.18em] text-[var(--color-text-secondary)]">
              Total
            </span>
            <span className="num text-2xl font-bold leading-tight tracking-[-0.025em] text-[var(--color-text)]">
              {formatCurrency(total)}
            </span>
          </div>
        </div>
        {/* Legend list */}
        <ul className="flex h-full min-w-[140px] flex-[1] flex-col justify-center gap-1.5 overflow-y-auto text-xs">
          {chartData.map((entry, index) => {
            const dim =
              selectedCategory && entry.category !== selectedCategory
                ? 'opacity-40'
                : 'opacity-100';
            return (
              <li
                key={entry.category}
                className={`flex items-center justify-between gap-2 rounded-md px-1.5 py-1 transition-opacity hover:bg-[var(--color-surface)] ${dim}`}
              >
                <button
                  type="button"
                  onClick={() => onCategoryClick?.(entry.category)}
                  className="flex min-w-0 flex-1 items-center gap-2 text-left"
                >
                  <span
                    className="inline-block h-2.5 w-2.5 shrink-0 rounded-sm"
                    style={{ backgroundColor: COLORS[index % COLORS.length] }}
                    aria-hidden="true"
                  />
                  <span className="truncate font-medium text-[var(--color-text)]">
                    {toTitleCase(entry.category)}
                  </span>
                </button>
                <span className="num shrink-0 text-[var(--color-text-secondary)]">
                  {entry.percentage.toFixed(0)}%
                </span>
              </li>
            );
          })}
        </ul>
        <span className="sr-only">{summary}</span>
      </div>

      {/* Subcategory drill-down donut */}
      {showSubchart && (
        <div className="flex min-h-0 flex-1 flex-col">
          <div className="mb-2 flex items-center justify-between px-1">
            <h4 className="text-[11px] font-semibold uppercase tracking-[0.15em] text-[var(--color-text-secondary)]">
              {toTitleCase(selectedCategory)} Breakdown
            </h4>
            <button
              onClick={onDismissSubcategory}
              className="text-xs text-[var(--color-text-secondary)] transition-colors hover:text-[var(--color-text)]"
              aria-label="Close subcategory breakdown"
            >
              Close ✕
            </button>
          </div>
          <div className="min-h-0 flex-1">
            <ResponsiveContainer width="100%" height="100%">
              <PieChart margin={{ top: 0, right: 0, bottom: 0, left: 0 }}>
                <Pie
                  data={subChartData}
                  cx="50%"
                  cy="50%"
                  innerRadius="55%"
                  outerRadius="88%"
                  paddingAngle={2}
                  dataKey="amount"
                  nameKey="subcategory"
                >
                  {subChartData.map((_, index) => (
                    <Cell
                      key={`sub-cell-${index}`}
                      fill={SUB_COLORS[index % SUB_COLORS.length]}
                      stroke="var(--color-card)"
                      strokeWidth={2}
                    />
                  ))}
                </Pie>
                <Tooltip content={<CustomTooltip />} />
                <Legend
                  verticalAlign="bottom"
                  iconType="circle"
                  formatter={(value: string, entry) => {
                    const item = subChartData.find((c) => c.subcategory === value);
                    const color =
                      'color' in entry && typeof entry.color === 'string'
                        ? entry.color
                        : 'var(--color-text)';
                    return (
                      <span style={{ color }} className="text-xs">
                        {toTitleCase(value)} {item ? `${item.percentage.toFixed(0)}%` : ''}
                      </span>
                    );
                  }}
                />
              </PieChart>
            </ResponsiveContainer>
          </div>
        </div>
      )}
    </div>
  );
}
