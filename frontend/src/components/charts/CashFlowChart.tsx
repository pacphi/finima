import {
  BarChart,
  Bar,
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
  const d = new Date(month + '-01');
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
    <div role="img" aria-label={summary}>
      <ResponsiveContainer width="100%" height={250}>
        <BarChart data={data} margin={{ top: 5, right: 20, left: 10, bottom: 5 }}>
          <CartesianGrid strokeDasharray="3 3" stroke="var(--color-border)" />
          <XAxis
            dataKey="month"
            tickFormatter={formatMonth}
            tick={{ fontSize: 11, fill: 'var(--color-text-secondary)' }}
          />
          <YAxis
            tickFormatter={(v: number) => formatCurrency(v)}
            tick={{ fontSize: 11, fill: 'var(--color-text-secondary)' }}
            width={80}
          />
          <Tooltip content={<CustomTooltip />} />
          <Legend />
          <Bar dataKey="income" name="Income" fill="#22C55E" radius={[2, 2, 0, 0]} />
          <Bar dataKey="expenses" name="Expenses" fill="#EF4444" radius={[2, 2, 0, 0]} />
        </BarChart>
      </ResponsiveContainer>
      <span className="sr-only">{summary}</span>
    </div>
  );
}
