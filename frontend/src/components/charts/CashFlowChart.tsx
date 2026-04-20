import {
  ComposedChart,
  Bar,
  Line,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
  Legend,
} from 'recharts';
import type { MonthlyCashFlow } from '@/types/models';

interface CashFlowChartProps {
  data: MonthlyCashFlow[];
}

function formatCurrency(value: number): string {
  return new Intl.NumberFormat('en-US', {
    style: 'currency',
    currency: 'USD',
    minimumFractionDigits: 0,
    maximumFractionDigits: 0,
  }).format(value);
}

function formatMonth(month: string): string {
  // Backend may return full date (YYYY-MM-DD) or month (YYYY-MM).
  // Normalize to YYYY-MM-DD before parsing.
  const normalized = month.length <= 7 ? month + '-01' : month;
  const d = new Date(normalized + 'T00:00:00');
  if (isNaN(d.getTime())) return month;
  return d.toLocaleDateString('en-US', { month: 'short' });
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

function buildSummary(data: MonthlyCashFlow[]): string {
  if (data.length === 0) return 'No cash flow data available.';
  const totalIncome = data.reduce((s, d) => s + d.income, 0);
  const totalExpenses = data.reduce((s, d) => s + d.expenses, 0);
  const latest = data[data.length - 1];
  const first = data[0];
  return `Cash flow chart showing ${data.length} months from ${first ? formatMonth(first.month) : ''} to ${latest ? formatMonth(latest.month) : ''}. Total income: ${formatCurrency(totalIncome)}. Total expenses: ${formatCurrency(totalExpenses)}. Net: ${formatCurrency(totalIncome - totalExpenses)}.`;
}

export function CashFlowChart({ data }: CashFlowChartProps) {
  const summary = buildSummary(data);

  return (
    <div className="h-full w-full" role="img" aria-label={summary}>
      <ResponsiveContainer width="100%" height="100%">
        <ComposedChart data={data} margin={{ top: 8, right: 16, left: 0, bottom: 4 }}>
          <CartesianGrid strokeDasharray="3 3" stroke="var(--color-border)" vertical={false} />
          <XAxis
            dataKey="month"
            tickFormatter={formatMonth}
            tick={{ fontSize: 11, fill: 'var(--color-text-secondary)' }}
            axisLine={false}
            tickLine={false}
          />
          <YAxis
            tickFormatter={(v: number) => formatCurrency(v)}
            tick={{ fontSize: 11, fill: 'var(--color-text-secondary)' }}
            width={72}
            axisLine={false}
            tickLine={false}
          />
          <Tooltip content={<CustomTooltip />} />
          <Legend iconType="circle" wrapperStyle={{ fontSize: 12 }} />
          <Bar dataKey="income" name="Income" fill="#22C55E" radius={[4, 4, 0, 0]} maxBarSize={28} />
          <Bar
            dataKey="expenses"
            name="Expenses"
            fill="#EF4444"
            radius={[4, 4, 0, 0]}
            maxBarSize={28}
          />
          <Line
            type="monotone"
            dataKey="net"
            name="Net"
            stroke="var(--color-primary)"
            strokeWidth={2}
            strokeDasharray="4 3"
            dot={{ r: 2, fill: 'var(--color-primary)', strokeWidth: 0 }}
            activeDot={{ r: 4 }}
          />
        </ComposedChart>
      </ResponsiveContainer>
      <span className="sr-only">{summary}</span>
    </div>
  );
}
