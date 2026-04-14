import type {
  Transaction,
  TransactionFilters,
  PaginationParams,
  SortParams,
  PaginatedResponse,
  CategorizeAccepted,
  CategorizationJobStatus,
} from '@/types/models';

export function createTransactionApi(api: {
  get: <T>(path: string) => Promise<T>;
  post: <T>(path: string, body?: unknown) => Promise<T>;
  put: <T>(path: string, body?: unknown) => Promise<T>;
}) {
  function buildQuery(
    filters?: TransactionFilters,
    pagination?: PaginationParams,
    sort?: SortParams,
  ): string {
    const params = new URLSearchParams();
    if (filters) {
      if (filters.date_from) params.set('date_from', filters.date_from);
      if (filters.date_to) params.set('date_to', filters.date_to);
      if (filters.account_id) params.set('account_id', filters.account_id);
      if (filters.category) params.set('category', filters.category);
      if (filters.search) params.set('search', filters.search);
      if (filters.amount_min !== undefined) params.set('amount_min', String(filters.amount_min));
      if (filters.amount_max !== undefined) params.set('amount_max', String(filters.amount_max));
    }
    if (pagination) {
      params.set('page', String(pagination.page));
      params.set('per_page', String(pagination.per_page));
    }
    if (sort) {
      params.set('sort_by', sort.sort_by);
      params.set('sort_dir', sort.sort_dir);
    }
    const qs = params.toString();
    return qs ? `?${qs}` : '';
  }

  return {
    listTransactions: (
      filters?: TransactionFilters,
      pagination?: PaginationParams,
      sort?: SortParams,
    ) =>
      api.get<PaginatedResponse<Transaction>>(
        `/api/transactions${buildQuery(filters, pagination, sort)}`,
      ),

    updateTransaction: (
      id: string,
      data: Partial<{ category: string; subcategory: string; notes: string; tags: string[] }>,
    ) => api.put<Transaction>(`/api/transactions/${id}`, data),

    bulkUpdateTransactions: (data: { ids: string[]; category: string }) =>
      api.post<{ updated: number }>('/api/transactions/bulk-update', data),

    searchTransactions: (query: string) =>
      api.get<Transaction[]>(`/api/transactions/search?q=${encodeURIComponent(query)}`),

    categorizeUncategorized: (accountId: string) =>
      api.post<CategorizeAccepted>('/api/transactions/categorize', {
        account_id: accountId,
      }),

    getCategorizeStatus: (accountId: string) =>
      api.get<CategorizationJobStatus>(
        `/api/transactions/categorize/status?account_id=${encodeURIComponent(accountId)}`,
      ),
  };
}
