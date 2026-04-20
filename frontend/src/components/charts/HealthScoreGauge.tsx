import type { HealthScore } from '@/types/models';

interface HealthScoreGaugeProps {
  data: HealthScore;
}

function getScoreLabel(score: number): string {
  if (score >= 90) return 'Excellent';
  if (score >= 75) return 'Great';
  if (score >= 60) return 'Good';
  if (score >= 40) return 'Fair';
  return 'Poor';
}

function getScoreColor(score: number): string {
  if (score >= 90) return '#22C55E';
  if (score >= 75) return '#22C55E';
  if (score >= 60) return '#3B82F6';
  if (score >= 40) return '#F59E0B';
  return '#EF4444';
}

export function HealthScoreGauge({ data }: HealthScoreGaugeProps) {
  const label = getScoreLabel(data.score);
  const color = getScoreColor(data.score);
  const circumference = 2 * Math.PI * 45;
  const dashOffset = circumference - (data.score / 100) * circumference;

  const trendLabel =
    data.spending_trend > 0 ? 'Increasing' : data.spending_trend < 0 ? 'Decreasing' : 'Stable';
  const summaryText = `Financial health score: ${data.score} out of 100 (${label}). Savings rate ${(data.savings_rate * 100).toFixed(0)}%, debt ratio ${(data.debt_ratio * 100).toFixed(0)}%, emergency fund ${data.emergency_months.toFixed(1)} months, spending trend ${trendLabel}.`;

  const stats: Array<{ label: string; value: string }> = [
    { label: 'Savings', value: `${(data.savings_rate * 100).toFixed(0)}%` },
    { label: 'Debt', value: `${(data.debt_ratio * 100).toFixed(0)}%` },
    { label: 'Emergency', value: `${data.emergency_months.toFixed(1)} mo` },
    { label: 'Trend', value: trendLabel },
  ];

  return (
    <div
      className="flex h-full w-full flex-col items-center justify-center gap-5"
      role="img"
      aria-label={summaryText}
    >
      <div className="relative aspect-square w-[min(60%,170px)]">
        <svg className="h-full w-full -rotate-90" viewBox="0 0 100 100" aria-hidden="true">
          <circle cx="50" cy="50" r="45" fill="none" stroke="var(--color-border)" strokeWidth="6" />
          <circle
            cx="50"
            cy="50"
            r="45"
            fill="none"
            stroke={color}
            strokeWidth="6"
            strokeLinecap="round"
            strokeDasharray={circumference}
            strokeDashoffset={dashOffset}
            className="transition-all duration-500"
          />
        </svg>
        <div className="absolute inset-0 flex flex-col items-center justify-center">
          <span
            className="num text-5xl font-bold leading-none tracking-[-0.03em]"
            style={{ color }}
          >
            {data.score}
          </span>
          <span className="mt-1 text-[11px] font-semibold uppercase tracking-[0.18em] text-[var(--color-text-secondary)]">
            {label}
          </span>
        </div>
      </div>
      <div className="grid w-full grid-cols-2 gap-x-4 gap-y-2" aria-hidden="true">
        {stats.map((s) => (
          <div
            key={s.label}
            className="flex items-baseline justify-between border-b border-[var(--color-border)]/60 pb-1.5 last:border-0"
          >
            <span className="text-[11px] uppercase tracking-wider text-[var(--color-text-secondary)]">
              {s.label}
            </span>
            <span className="num text-sm font-semibold text-[var(--color-text)]">{s.value}</span>
          </div>
        ))}
      </div>
      <span className="sr-only">{summaryText}</span>
    </div>
  );
}
