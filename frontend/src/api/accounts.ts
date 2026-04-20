import type { Account, AccountType, SignConvention, SignOverrideResponse } from '@/types/models';

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
        portfolio_id: string;
      }>,
    ) => api.put<Account>(`/api/accounts/${id}`, data),

    archiveAccount: (id: string) => api.del<void>(`/api/accounts/${id}`),

    /** **Dangerous.** Permanently delete the account and every associated
     *  transaction, upload, and stored file. Cannot be undone. */
    deleteAccount: (id: string) => api.del<void>(`/api/accounts/${id}/purge`),

    setPrimary: (id: string) => api.post<Account>(`/api/accounts/${id}/set-primary`),

    /** Fetch the account's balance-at-end-of-bucket series across its full
     *  transaction history. Bucketing and running sums run in Postgres, so
     *  the response is already chart-sized (~12–60 points). Pass
     *  `bucket="auto"` (default) to let the server pick granularity from the
     *  txn span; override with daily/weekly/monthly/yearly when the UI
     *  exposes a toggle. */
    getBalanceHistory: (
      id: string,
      bucket: 'auto' | 'daily' | 'weekly' | 'monthly' | 'yearly' = 'auto',
    ) =>
      api.get<{
        bucket: 'daily' | 'weekly' | 'monthly' | 'yearly';
        points: { date: string; balance: number | string }[];
      }>(`/api/accounts/${id}/balance-history?bucket=${bucket}`),

    /** Set or clear the per-account sign-convention override. Pass
     *  `null` to clear and fall back to the default resolution chain.
     *  Triggers server-side re-normalization of every transaction on
     *  the account — both `direction` and the canonical `amount`
     *  sign. See ADR-018.
     *
     *  The server returns the refreshed AccountDetailResponse (the
     *  plain account row plus `computed_balance`,
     *  `transaction_count`, `last_import_at`); we map it the same
     *  way `getAccount` does so the caller can drop the account
     *  straight into component state. */
    setSignOverride: async (
      id: string,
      convention: SignConvention | null,
    ): Promise<SignOverrideResponse> => {
      const raw = await api.put<
        Omit<SignOverrideResponse, 'account'> & {
          account: Account & { computed_balance?: number };
        }
      >(`/api/accounts/${id}/sign-override`, { convention });
      const a = raw.account;
      return {
        ...raw,
        account: {
          ...a,
          current_balance: a.computed_balance ?? a.current_balance ?? a.opening_balance ?? 0,
          transaction_count: a.transaction_count ?? 0,
          last_import_at: a.last_import_at ?? null,
          updated_at: a.updated_at ?? a.created_at,
        },
      };
    },
  };
}
