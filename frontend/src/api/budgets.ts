import type { Budget, BudgetVsActual, BudgetSuggestion } from '@/types/models';

function suffix(portfolioId: string | null): string {
  return portfolioId ? `&portfolio_id=${encodeURIComponent(portfolioId)}` : '';
}

export function createBudgetApi(api: {
  get: <T>(path: string) => Promise<T>;
  post: <T>(path: string, body?: unknown) => Promise<T>;
  put: <T>(path: string, body?: unknown) => Promise<T>;
}) {
  return {
    listBudgets: (month: string, portfolioId: string | null = null) =>
      api.get<Budget[]>(`/api/budgets?month=${month}${suffix(portfolioId)}`),

    createBudget: (data: {
      portfolio_id?: string | null;
      category: string;
      amount: number;
      month: string;
    }) => {
      const body: Record<string, unknown> = { ...data };
      // Strip null/empty portfolio_id so the server falls back to the
      // user's first portfolio (instead of failing UUID parse on "").
      if (!body.portfolio_id) delete body.portfolio_id;
      return api.post<Budget>('/api/budgets', body);
    },

    updateBudget: (id: string, data: Partial<{ amount: number; category: string }>) =>
      api.put<Budget>(`/api/budgets/${id}`, data),

    getBudgetVsActual: (month: string, portfolioId: string | null = null) =>
      api.get<BudgetVsActual[]>(`/api/budgets/vs-actual?month=${month}${suffix(portfolioId)}`),

    autoSuggestBudgets: (portfolioId: string | null = null) =>
      api.post<BudgetSuggestion[]>(
        portfolioId
          ? `/api/budgets/auto-suggest?portfolio_id=${encodeURIComponent(portfolioId)}`
          : '/api/budgets/auto-suggest',
      ),
  };
}
