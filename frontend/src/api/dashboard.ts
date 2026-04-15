import type {
  DashboardSummary,
  NetWorthPoint,
  MonthlyCashFlow,
  CategorySpend,
  SubcategorySpend,
  HealthScore,
} from '@/types/models';

export function createDashboardApi(api: { get: <T>(path: string) => Promise<T> }) {
  return {
    getDashboardSummary: () => api.get<DashboardSummary>('/api/dashboard/summary'),

    getNetWorth: (months: number = 12) =>
      api.get<NetWorthPoint[]>(`/api/dashboard/net-worth?months=${months}`),

    getCashflow: (months: number = 12) =>
      api.get<MonthlyCashFlow[]>(`/api/dashboard/cashflow?months=${months}`),

    getSpending: (month?: string) =>
      api.get<CategorySpend[]>(
        month ? `/api/dashboard/spending?month=${month}` : '/api/dashboard/spending',
      ),

    getSubcategorySpending: (category: string, month?: string) => {
      const params = new URLSearchParams({ category });
      if (month) params.set('month', month);
      return api.get<SubcategorySpend[]>(
        `/api/dashboard/spending/subcategories?${params.toString()}`,
      );
    },

    getHealthScore: () => api.get<HealthScore>('/api/dashboard/health-score'),
  };
}
