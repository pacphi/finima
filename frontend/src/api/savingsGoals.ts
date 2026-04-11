import type { SavingsGoal } from '@/types/models';

export function createSavingsGoalApi(api: {
  get: <T>(path: string) => Promise<T>;
  post: <T>(path: string, body?: unknown) => Promise<T>;
  put: <T>(path: string, body?: unknown) => Promise<T>;
  del: <T>(path: string) => Promise<T>;
}) {
  return {
    listGoals: () => api.get<SavingsGoal[]>('/api/savings-goals'),

    createGoal: (data: {
      name: string;
      target_amount: number;
      target_date?: string;
      monthly_contribution?: number;
    }) => api.post<SavingsGoal>('/api/savings-goals', data),

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
