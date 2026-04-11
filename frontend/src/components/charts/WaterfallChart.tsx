import {
  BarChart,
  Bar,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
  Cell,
  LabelList,
} from 'recharts';
import type { WaterfallData } from '@/types/models';

interface WaterfallChartProps {
  data: WaterfallData;
}

interface WaterfallBar {
  name: string;
  base: number;
  value: number;
  display: number;
  color: string;
  type: 'start' | 'income' | 'outflow' | 'end';
}

function formatCurrency(value: number): string {
  return new Intl.NumberFormat('en-US', {
    style: 'currency',
    currency: 'USD',
    minimumFractionDigits: 0,
    maximumFractionDigits: 0,
  }).format(value);
}

function formatShort(value: number): string {
  if (Math.abs(value) >= 1000) {
    return `$${(value / 1000).toFixed(1)}k`;
  }
  return `$${value.toFixed(0)}`;
}

function buildWaterfallBars(data: WaterfallData): WaterfallBar[] {
  const bars: WaterfallBar[] = [];
  let running = data.start_balance;

  bars.push({
    name: 'Start',
    base: 0,
    value: data.start_balance,
    display: data.start_balance,
    color: '#3B82F6',
    type: 'start',
  });

  running += data.income;
  bars.push({
    name: 'Income',
    base: running - data.income,
    value: data.income,
    display: data.income,
    color: '#22C55E',
    type: 'income',
  });

  for (const outflow of data.outflows) {
    bars.push({
      name: outflow.name,
      base: running - outflow.amount,
      value: outflow.amount,
      display: -outflow.amount,
      color: '#EF4444',
      type: 'outflow',
    });
    running -= outflow.amount;
  }

  bars.push({
    name: 'End',
    base: 0,
    value: data.end_balance,
    display: data.end_balance,
    color: '#3B82F6',
    type: 'end',
  });

  return bars;
}

interface TooltipPayloadEntry {
  payload: WaterfallBar;
}

function CustomTooltip({ active, payload }: { active?: boolean; payload?: TooltipPayloadEntry[] }) {
  if (!active || !payload?.length) return null;
  const bar = payload[0]?.payload;
  if (!bar) return null;
  return (
    <div className="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] p-3 shadow-lg">
      <p className="text-sm font-medium text-[var(--color-text)]">{bar.name}</p>
      <p className="text-sm" style={{ color: bar.color }}>
        {bar.type === 'outflow' ? '-' : ''}
        {formatCurrency(Math.abs(bar.display))}
      </p>
    </div>
  );
}

function RenderLabel(props: { x?: number; y?: number; width?: number; value?: number }) {
  const { x = 0, y = 0, width: w = 0, value } = props;
  if (!value) return null;
  return (
    <text
      x={x + w / 2}
      y={y - 6}
      textAnchor="middle"
      fill="var(--color-text-secondary)"
      fontSize={10}
    >
      {formatShort(value)}
    </text>
  );
}

export function WaterfallChart({ data }: WaterfallChartProps) {
  const bars = buildWaterfallBars(data);
  const totalOutflows = data.outflows.reduce((s, o) => s + o.amount, 0);
  const netChange = data.end_balance - data.start_balance;

  const summaryText = `Waterfall chart: Started at ${formatCurrency(data.start_balance)}, received ${formatCurrency(data.income)} income, paid out ${formatCurrency(totalOutflows)}, ended at ${formatCurrency(data.end_balance)} (net ${netChange >= 0 ? '+' : ''}${formatCurrency(netChange)} for the month).`;

  return (
    <div role="img" aria-label={summaryText}>
      <ResponsiveContainer width="100%" height={320}>
        <BarChart data={bars} margin={{ top: 20, right: 20, left: 10, bottom: 5 }}>
          <CartesianGrid strokeDasharray="3 3" stroke="var(--color-border)" />
          <XAxis dataKey="name" tick={{ fontSize: 11, fill: 'var(--color-text-secondary)' }} />
          <YAxis
            tickFormatter={(v: number) => formatShort(v)}
            tick={{ fontSize: 11, fill: 'var(--color-text-secondary)' }}
            width={60}
          />
          <Tooltip content={<CustomTooltip />} />
          {/* Invisible base bar for stacking */}
          <Bar dataKey="base" stackId="stack" fill="transparent" />
          {/* Visible value bar */}
          <Bar dataKey="value" stackId="stack" radius={[2, 2, 0, 0]}>
            {bars.map((bar, index) => (
              <Cell key={index} fill={bar.color} />
            ))}
            <LabelList dataKey="display" position="top" content={<RenderLabel />} />
          </Bar>
        </BarChart>
      </ResponsiveContainer>
      <p className="mt-3 text-sm text-[var(--color-text-secondary)]">
        Started at {formatCurrency(data.start_balance)} → received {formatCurrency(data.income)}{' '}
        income → paid out {formatCurrency(totalOutflows)} → ended at{' '}
        {formatCurrency(data.end_balance)} (net {netChange >= 0 ? '+' : ''}
        {formatCurrency(netChange)} for the month)
      </p>
    </div>
  );
}
