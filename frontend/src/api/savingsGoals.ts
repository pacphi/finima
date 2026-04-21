import type { SavingsGoal } from '@/types/models';

function withPortfolio(path: string, portfolioId: string | null): string {
  if (!portfolioId) return path;
  const sep = path.includes('?') ? '&' : '?';
  return `${path}${sep}portfolio_id=${encodeURIComponent(portfolioId)}`;
}

export function createSavingsGoalApi(api: {
  get: <T>(path: string) => Promise<T>;
  post: <T>(path: string, body?: unknown) => Promise<T>;
  put: <T>(path: string, body?: unknown) => Promise<T>;
  del: <T>(path: string) => Promise<T>;
}) {
  return {
    listGoals: (portfolioId: string | null = null) =>
      api.get<SavingsGoal[]>(withPortfolio('/api/savings-goals', portfolioId)),

    createGoal: (data: {
      name: string;
      target_amount: number;
      target_date?: string;
      monthly_contribution?: number;
      portfolio_id?: string | null;
    }) => {
      const body = { ...data };
      if (body.portfolio_id == null) delete body.portfolio_id;
      return api.post<SavingsGoal>('/api/savings-goals', body);
    },

    updateGoal: (
      id: string,
      data: Partial<{
        name: string;
        target_amount: number;
        current_amount: number;
        target_date: string;
        monthly_contribution: number;
      }>,
    ) => api.put<SavingsGoal>(`/api/savings-goals/${id}`, data),

    deleteGoal: (id: string) => api.del<void>(`/api/savings-goals/${id}`),
  };
}
