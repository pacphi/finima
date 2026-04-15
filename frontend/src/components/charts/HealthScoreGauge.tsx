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

  return (
    <div className="flex flex-col items-center gap-4" role="img" aria-label={summaryText}>
      <div className="relative h-32 w-32">
        <svg className="h-32 w-32 -rotate-90" viewBox="0 0 100 100" aria-hidden="true">
          <circle cx="50" cy="50" r="45" fill="none" stroke="var(--color-border)" strokeWidth="8" />
          <circle
            cx="50"
            cy="50"
            r="45"
            fill="none"
            stroke={color}
            strokeWidth="8"
            strokeLinecap="round"
            strokeDasharray={circumference}
            strokeDashoffset={dashOffset}
            className="transition-all duration-500"
          />
        </svg>
        <div className="absolute inset-0 flex flex-col items-center justify-center">
          <span className="text-3xl font-bold" style={{ color }}>
            {data.score}
          </span>
          <span className="text-xs text-[var(--color-text-secondary)]">{label}</span>
        </div>
      </div>
      <div className="grid w-full grid-cols-2 gap-x-4 gap-y-2 text-sm" aria-hidden="true">
        <div className="flex justify-between">
          <span className="text-[var(--color-text-secondary)]">Savings Rate</span>
          <span className="font-medium text-[var(--color-text)]">
            {(data.savings_rate * 100).toFixed(0)}%
          </span>
        </div>
        <div className="flex justify-between">
          <span className="text-[var(--color-text-secondary)]">Debt Ratio</span>
          <span className="font-medium text-[var(--color-text)]">
            {(data.debt_ratio * 100).toFixed(0)}%
          </span>
        </div>
        <div className="flex justify-between">
          <span className="text-[var(--color-text-secondary)]">Emergency</span>
          <span className="font-medium text-[var(--color-text)]">
            {data.emergency_months.toFixed(1)} mo
          </span>
        </div>
        <div className="flex justify-between">
          <span className="text-[var(--color-text-secondary)]">Trend</span>
          <span className="font-medium text-[var(--color-text)]">{trendLabel}</span>
        </div>
      </div>
      {/* Visually hidden text summary for screen readers (supplements the grid) */}
      <span className="sr-only">{summaryText}</span>
    </div>
  );
}
