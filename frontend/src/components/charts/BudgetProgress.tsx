import type { BudgetVsActual } from '@/types/models';

interface BudgetProgressProps {
  data: BudgetVsActual;
}

function formatCurrency(value: number): string {
  return new Intl.NumberFormat('en-US', {
    style: 'currency',
    currency: 'USD',
    minimumFractionDigits: 0,
    maximumFractionDigits: 0,
  }).format(value);
}

function getBarColor(percentage: number): string {
  if (percentage > 100) return '#EF4444';
  if (percentage >= 80) return '#F59E0B';
  return '#22C55E';
}

function getStatusText(percentage: number): string {
  if (percentage > 100) return 'over budget';
  if (percentage >= 80) return 'near limit';
  return 'on track';
}

export function BudgetProgress({ data }: BudgetProgressProps) {
  const pct = Math.min(data.percentage, 120);
  const color = getBarColor(data.percentage);
  const statusText = getStatusText(data.percentage);
  const progressLabel = `${data.category}: ${data.percentage.toFixed(0)}% of budget used (${statusText}). ${formatCurrency(data.spent)} spent of ${formatCurrency(data.limit)}.`;

  return (
    <div className="space-y-1">
      <div className="flex items-center justify-between text-sm">
        <span className="font-medium text-[var(--color-text)]">{data.category}</span>
        <span className="text-[var(--color-text-secondary)]">
          {formatCurrency(data.spent)} / {formatCurrency(data.limit)}
        </span>
      </div>
      <div
        className="h-2.5 w-full overflow-hidden rounded-full bg-[var(--color-border)]"
        role="progressbar"
        aria-valuenow={Math.round(data.percentage)}
        aria-valuemin={0}
        aria-valuemax={100}
        aria-label={progressLabel}
      >
        <div
          className="h-full rounded-full transition-all duration-300"
          style={{
            width: `${Math.min(pct, 100)}%`,
            backgroundColor: color,
          }}
        />
      </div>
      <div className="flex justify-between text-xs text-[var(--color-text-secondary)]">
        <span>
          {data.remaining >= 0
            ? `${formatCurrency(data.remaining)} remaining`
            : `${formatCurrency(Math.abs(data.remaining))} over budget`}
        </span>
        <span>
          {data.percentage.toFixed(0)}% used ({statusText})
        </span>
      </div>
    </div>
  );
}
