import type {
  AccountFlow,
  FlowGroup,
  SankeyData,
  OutflowRank,
  WaterfallData,
} from '@/types/models';

function withPortfolio(path: string, portfolioId: string | null): string {
  if (!portfolioId) return path;
  const sep = path.includes('?') ? '&' : '?';
  return `${path}${sep}portfolio_id=${encodeURIComponent(portfolioId)}`;
}

export function createFlowApi(api: {
  get: <T>(path: string) => Promise<T>;
  post: <T>(path: string, body?: unknown) => Promise<T>;
  put: <T>(path: string, body?: unknown) => Promise<T>;
  del: <T>(path: string) => Promise<T>;
}) {
  return {
    listFlows: (month: string, portfolioId: string | null = null) =>
      api.get<AccountFlow[]>(withPortfolio(`/api/flows?month=${month}`, portfolioId)),

    createFlow: (data: {
      source_account_id: string;
      destination_account_id: string;
      amount: number;
      date: string;
      source_transaction_id?: string;
      destination_transaction_id?: string;
      portfolio_id?: string | null;
    }) => {
      const body: Record<string, unknown> = { ...data };
      if (!body.portfolio_id) delete body.portfolio_id;
      return api.post<AccountFlow>('/api/flows', body);
    },

    confirmFlow: (id: string) => api.put<AccountFlow>(`/api/flows/${id}`, { action: 'confirm' }),

    dismissFlow: (id: string) => api.put<AccountFlow>(`/api/flows/${id}`, { action: 'dismiss' }),

    deleteFlow: (id: string) => api.del<void>(`/api/flows/${id}`),

    detectFlows: (month: string, portfolioId: string | null = null) =>
      api.post<{ detected: number; created: number }>(
        withPortfolio(`/api/flows/detect?month=${month}`, portfolioId),
      ),

    getSankeyData: (month: string, portfolioId: string | null = null) =>
      api.get<SankeyData>(withPortfolio(`/api/flows/sankey?month=${month}`, portfolioId)),

    getFullSankeyData: (month: string, portfolioId: string | null = null) =>
      api.get<SankeyData>(withPortfolio(`/api/flows/sankey-full?month=${month}`, portfolioId)),

    getOutflowRanking: (month: string, portfolioId: string | null = null) =>
      api.get<OutflowRank[]>(
        withPortfolio(`/api/flows/outflow-ranking?month=${month}`, portfolioId),
      ),

    getBalanceImpact: (month: string, accountId: string, portfolioId: string | null = null) =>
      api.get<WaterfallData>(
        withPortfolio(
          `/api/flows/balance-impact?month=${month}&account_id=${accountId}`,
          portfolioId,
        ),
      ),

    listFlowGroups: (portfolioId: string | null = null) =>
      api.get<FlowGroup[]>(withPortfolio('/api/flow-groups', portfolioId)),

    createFlowGroup: (name: string, portfolioId: string | null = null) => {
      const body: Record<string, unknown> = { name };
      if (portfolioId) body.portfolio_id = portfolioId;
      return api.post<FlowGroup>('/api/flow-groups', body);
    },

    updateFlowGroup: (id: string, name: string) =>
      api.put<FlowGroup>(`/api/flow-groups/${id}`, { name }),

    deleteFlowGroup: (id: string) => api.del<void>(`/api/flow-groups/${id}`),
  };
}
