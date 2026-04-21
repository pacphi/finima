import type { Portfolio } from '@/types/models';

export function createPortfolioApi(api: {
  get: <T>(path: string) => Promise<T>;
  post: <T>(path: string, body?: unknown) => Promise<T>;
  put: <T>(path: string, body?: unknown) => Promise<T>;
  del: <T>(path: string) => Promise<T>;
}) {
  return {
    listPortfolios: () => api.get<Portfolio[]>('/api/portfolios'),

    createPortfolio: (data: { name: string; description?: string }) =>
      api.post<Portfolio>('/api/portfolios', data),

    getPortfolio: (id: string) => api.get<Portfolio>(`/api/portfolios/${id}`),

    updatePortfolio: (id: string, data: { name?: string; description?: string }) =>
      api.put<Portfolio>(`/api/portfolios/${id}`, data),

    /** **Dangerous.** Permanently delete the portfolio and every associated
     *  account, transaction, upload, budget, goal, and stored file.
     *  Cannot be undone. */
    deletePortfolio: (id: string) => api.del<void>(`/api/portfolios/${id}`),
  };
}
