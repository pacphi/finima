import type { Account, AccountType } from '@/types/models';

export function createAccountApi(api: {
  get: <T>(path: string) => Promise<T>;
  post: <T>(path: string, body?: unknown) => Promise<T>;
  put: <T>(path: string, body?: unknown) => Promise<T>;
  del: <T>(path: string) => Promise<T>;
}) {
  return {
    listAccounts: (portfolioId: string) =>
      api.get<Account[]>(`/api/portfolios/${portfolioId}/accounts`),

    createAccount: (data: {
      portfolio_id: string;
      name: string;
      account_type: AccountType;
      institution?: string;
      currency?: string;
      opening_balance?: number;
      notes?: string;
    }) => api.post<Account>('/api/accounts', data),

    getAccount: (id: string) => api.get<Account>(`/api/accounts/${id}`),

    updateAccount: (
      id: string,
      data: Partial<{
        name: string;
        account_type: AccountType;
        institution: string;
        currency: string;
        is_primary_income: boolean;
        notes: string;
      }>,
    ) => api.put<Account>(`/api/accounts/${id}`, data),

    archiveAccount: (id: string) => api.del<void>(`/api/accounts/${id}`),
  };
}
