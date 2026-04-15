export interface PayeeSummary {
  merchant_name: string;
  category: string | null;
  subcategory: string | null;
  transaction_count: number;
}

export interface ApplyPayeeRuleResponse {
  updated: number;
}

export function createPayeeRulesApi(api: {
  get: <T>(path: string) => Promise<T>;
  post: <T>(path: string, body?: unknown) => Promise<T>;
}) {
  return {
    listPayeeRules: (portfolioId: string) =>
      api.get<PayeeSummary[]>(`/api/payee-rules?portfolio_id=${portfolioId}`),

    applyPayeeRule: (data: {
      portfolio_id: string;
      merchant_name: string;
      new_category: string;
      new_subcategory?: string;
    }) => api.post<ApplyPayeeRuleResponse>('/api/payee-rules/apply', data),
  };
}
