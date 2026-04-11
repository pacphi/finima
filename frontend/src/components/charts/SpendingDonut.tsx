import { PieChart, Pie, Cell, ResponsiveContainer, Tooltip, Legend } from 'recharts';
import type { CategorySpend } from '@/types/models';

interface SpendingDonutProps {
  data: CategorySpend[];
  onCategoryClick?: (category: string) => void;
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
      <p className="text-sm font-medium text-[var(--color-text)]">{entry.name}</p>
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

export function SpendingDonut({ data, onCategoryClick }: SpendingDonutProps) {
  // Show top 5 categories + group the rest as "Other"
  const sorted = [...data].sort((a, b) => b.amount - a.amount);
  const top5 = sorted.slice(0, 5);
  const rest = sorted.slice(5);

  const chartData =
    rest.length > 0
      ? [
          ...top5,
          {
            category: 'Other',
            amount: rest.reduce((sum, c) => sum + c.amount, 0),
            percentage: rest.reduce((sum, c) => sum + c.percentage, 0),
          },
        ]
      : top5;

  const summary = buildSummary(data);

  return (
    <div role="img" aria-label={summary}>
      <ResponsiveContainer width="100%" height={280}>
        <PieChart>
          <Pie
            data={chartData}
            cx="50%"
            cy="45%"
            innerRadius={55}
            outerRadius={90}
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
            {chartData.map((_, index) => (
              <Cell key={`cell-${index}`} fill={COLORS[index % COLORS.length]} />
            ))}
          </Pie>
          <Tooltip content={<CustomTooltip />} />
          <Legend
            verticalAlign="bottom"
            formatter={(value: string, entry) => {
              const item = chartData.find((c) => c.category === value);
              const color =
                'color' in entry && typeof entry.color === 'string'
                  ? entry.color
                  : 'var(--color-text)';
              return (
                <span style={{ color }} className="text-xs">
                  {value} {item ? `${item.percentage.toFixed(0)}%` : ''}
                </span>
              );
            }}
          />
        </PieChart>
      </ResponsiveContainer>
      <span className="sr-only">{summary}</span>
    </div>
  );
}
