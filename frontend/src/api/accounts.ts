import type { Account, AccountType } from '@/types/models';

export function createAccountApi(api: {
  get: <T>(path: string) => Promise<T>;
  post: <T>(path: string, body?: unknown) => Promise<T>;
  put: <T>(path: string, body?: unknown) => Promise<T>;
  del: <T>(path: string) => Promise<T>;
}) {
  return {
    listAccounts: async (portfolioId: string): Promise<Account[]> => {
      const raw = await api.get<(Account & { computed_balance?: number })[]>(
        `/api/accounts?portfolio_id=${portfolioId}`,
      );
      return raw.map((a) => ({
        ...a,
        current_balance: a.computed_balance ?? a.current_balance ?? a.opening_balance ?? 0,
        transaction_count: a.transaction_count ?? 0,
        last_import_at: a.last_import_at ?? null,
        updated_at: a.updated_at ?? a.created_at,
      }));
    },

    createAccount: async (data: {
      portfolio_id: string;
      name: string;
      account_type: AccountType;
      institution?: string;
      currency?: string;
      opening_balance?: number;
      notes?: string;
    }): Promise<Account> => {
      const a = await api.post<Account>('/api/accounts', data);
      return {
        ...a,
        current_balance: a.current_balance ?? a.opening_balance ?? 0,
        transaction_count: a.transaction_count ?? 0,
        last_import_at: a.last_import_at ?? null,
        updated_at: a.updated_at ?? a.created_at,
      };
    },

    getAccount: async (id: string): Promise<Account> => {
      const a = await api.get<Account & { computed_balance?: number }>(`/api/accounts/${id}`);
      return {
        ...a,
        current_balance: a.computed_balance ?? a.current_balance ?? a.opening_balance ?? 0,
        transaction_count: a.transaction_count ?? 0,
        last_import_at: a.last_import_at ?? null,
        updated_at: a.updated_at ?? a.created_at,
      };
    },

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
