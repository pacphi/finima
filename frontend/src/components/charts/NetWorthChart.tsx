import {
  AreaChart,
  Area,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
} from 'recharts';
import type { NetWorthPoint } from '@/types/models';

interface NetWorthChartProps {
  data: NetWorthPoint[];
}

function formatCurrency(value: number): string {
  return new Intl.NumberFormat('en-US', {
    style: 'currency',
    currency: 'USD',
    minimumFractionDigits: 0,
    maximumFractionDigits: 0,
  }).format(value);
}

function formatMonth(dateStr: string): string {
  const d = new Date(dateStr);
  return d.toLocaleDateString('en-US', { month: 'short', year: '2-digit' });
}

interface TooltipPayloadEntry {
  name: string;
  value: number;
  color: string;
}

function CustomTooltip({
  active,
  payload,
  label,
}: {
  active?: boolean;
  payload?: TooltipPayloadEntry[];
  label?: string;
}) {
  if (!active || !payload?.length) return null;
  return (
    <div className="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] p-3 shadow-lg">
      <p className="text-xs text-[var(--color-text-secondary)]">
        {label ? formatMonth(label) : ''}
      </p>
      {payload.map((entry) => (
        <p key={entry.name} className="text-sm font-medium" style={{ color: entry.color }}>
          {entry.name}: {formatCurrency(entry.value)}
        </p>
      ))}
    </div>
  );
}

function buildSummary(data: NetWorthPoint[]): string {
  if (data.length === 0) return 'No net worth data available.';
  const first = data[0]!;
  const last = data[data.length - 1]!;
  const change = last.total - first.total;
  const direction = change >= 0 ? 'increased' : 'decreased';
  return `Net worth trend chart over ${data.length} data points. Started at ${formatCurrency(first.total)}, ended at ${formatCurrency(last.total)}. Net worth ${direction} by ${formatCurrency(Math.abs(change))}.`;
}

export function NetWorthChart({ data }: NetWorthChartProps) {
  const summary = buildSummary(data);

  return (
    <div role="img" aria-label={summary}>
      <ResponsiveContainer width="100%" height={250}>
        <AreaChart data={data} margin={{ top: 5, right: 20, left: 10, bottom: 5 }}>
          <defs>
            <linearGradient id="netWorthGradient" x1="0" y1="0" x2="0" y2="1">
              <stop offset="5%" stopColor="var(--color-accent, #3B82F6)" stopOpacity={0.3} />
              <stop offset="95%" stopColor="var(--color-accent, #3B82F6)" stopOpacity={0} />
            </linearGradient>
          </defs>
          <CartesianGrid strokeDasharray="3 3" stroke="var(--color-border)" />
          <XAxis
            dataKey="date"
            tickFormatter={formatMonth}
            tick={{ fontSize: 11, fill: 'var(--color-text-secondary)' }}
          />
          <YAxis
            tickFormatter={(v: number) => formatCurrency(v)}
            tick={{ fontSize: 11, fill: 'var(--color-text-secondary)' }}
            width={80}
          />
          <Tooltip content={<CustomTooltip />} />
          <Area
            type="monotone"
            dataKey="total"
            name="Net Worth"
            stroke="var(--color-accent, #3B82F6)"
            strokeWidth={2}
            fill="url(#netWorthGradient)"
          />
        </AreaChart>
      </ResponsiveContainer>
      <span className="sr-only">{summary}</span>
    </div>
  );
}
