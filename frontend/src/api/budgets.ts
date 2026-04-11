import type { Budget, BudgetVsActual, BudgetSuggestion } from '@/types/models';

export function createBudgetApi(api: {
  get: <T>(path: string) => Promise<T>;
  post: <T>(path: string, body?: unknown) => Promise<T>;
  put: <T>(path: string, body?: unknown) => Promise<T>;
}) {
  return {
    listBudgets: (month: string) => api.get<Budget[]>(`/api/budgets?month=${month}`),

    createBudget: (data: {
      portfolio_id: string;
      category: string;
      amount: number;
      month: string;
    }) => api.post<Budget>('/api/budgets', data),

    updateBudget: (id: string, data: Partial<{ amount: number; category: string }>) =>
      api.put<Budget>(`/api/budgets/${id}`, data),

    getBudgetVsActual: (month: string) =>
      api.get<BudgetVsActual[]>(`/api/budgets/vs-actual?month=${month}`),

    autoSuggestBudgets: () => api.post<BudgetSuggestion[]>('/api/budgets/auto-suggest'),
  };
}
