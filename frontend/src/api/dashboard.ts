import type {
  DashboardSummary,
  NetWorthPoint,
  MonthlyCashFlow,
  CategorySpend,
  SubcategorySpend,
  HealthScore,
} from '@/types/models';

/**
 * Build a querystring including the active portfolio id so dashboard
 * endpoints scope their response to the selected portfolio rather than
 * the user's first one. `portfolioId` is nullable during the initial
 * render (before the portfolio store hydrates); in that case we fall
 * back to omitting the param, preserving legacy behavior.
 */
function qs(portfolioId: string | null, extra: Record<string, string> = {}): string {
  const params = new URLSearchParams(extra);
  if (portfolioId) params.set('portfolio_id', portfolioId);
  const s = params.toString();
  return s ? `?${s}` : '';
}

export function createDashboardApi(api: { get: <T>(path: string) => Promise<T> }) {
  return {
    getDashboardSummary: (portfolioId: string | null = null) =>
      api.get<DashboardSummary>(`/api/dashboard/summary${qs(portfolioId)}`),

    getNetWorth: (months: number = 12, portfolioId: string | null = null) =>
      api.get<NetWorthPoint[]>(
        `/api/dashboard/net-worth${qs(portfolioId, { months: String(months) })}`,
      ),

    getCashflow: (months: number = 12, portfolioId: string | null = null) =>
      api.get<MonthlyCashFlow[]>(
        `/api/dashboard/cashflow${qs(portfolioId, { months: String(months) })}`,
      ),

    getSpending: (month?: string, portfolioId: string | null = null) => {
      const extra: Record<string, string> = {};
      if (month) extra.month = month;
      return api.get<CategorySpend[]>(`/api/dashboard/spending${qs(portfolioId, extra)}`);
    },

    getSubcategorySpending: (
      category: string,
      month?: string,
      portfolioId: string | null = null,
    ) => {
      const extra: Record<string, string> = { category };
      if (month) extra.month = month;
      return api.get<SubcategorySpend[]>(
        `/api/dashboard/spending/subcategories${qs(portfolioId, extra)}`,
      );
    },

    getHealthScore: (portfolioId: string | null = null) =>
      api.get<HealthScore>(`/api/dashboard/health-score${qs(portfolioId)}`),
  };
}
