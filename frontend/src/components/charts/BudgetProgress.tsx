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
    <div className="space-y-1.5">
      <div className="flex items-baseline justify-between gap-4 text-sm">
        <span className="truncate font-medium text-[var(--color-text)]">{data.category}</span>
        <span className="num shrink-0 text-[var(--color-text-secondary)]">
          <span className="font-semibold text-[var(--color-text)]">
            {formatCurrency(data.spent)}
          </span>
          <span className="mx-1 opacity-50">/</span>
          {formatCurrency(data.limit)}
        </span>
      </div>
      <div
        className="relative h-2 w-full overflow-hidden rounded-full bg-[var(--color-border)]/60"
        role="progressbar"
        aria-valuenow={Math.round(data.percentage)}
        aria-valuemin={0}
        aria-valuemax={100}
        aria-label={progressLabel}
      >
        <div
          className="h-full rounded-full transition-all duration-500 ease-out"
          style={{
            width: `${Math.min(pct, 100)}%`,
            backgroundColor: color,
            boxShadow: `0 0 0 1px ${color}33`,
          }}
        />
      </div>
      <div className="flex justify-between text-[11px] text-[var(--color-text-secondary)]">
        <span className="num">
          {data.remaining >= 0
            ? `${formatCurrency(data.remaining)} left`
            : `${formatCurrency(Math.abs(data.remaining))} over`}
        </span>
        <span className="num">
          {data.percentage.toFixed(0)}% · {statusText}
        </span>
      </div>
    </div>
  );
}
