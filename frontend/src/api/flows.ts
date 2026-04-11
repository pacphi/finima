import type {
  AccountFlow,
  FlowGroup,
  SankeyData,
  OutflowRank,
  WaterfallData,
} from '@/types/models';

export function createFlowApi(api: {
  get: <T>(path: string) => Promise<T>;
  post: <T>(path: string, body?: unknown) => Promise<T>;
  put: <T>(path: string, body?: unknown) => Promise<T>;
  del: <T>(path: string) => Promise<T>;
}) {
  return {
    listFlows: (month: string) => api.get<AccountFlow[]>(`/api/flows?month=${month}`),

    createFlow: (data: {
      source_account_id: string;
      destination_account_id: string;
      amount: number;
      date: string;
      source_transaction_id?: string;
      destination_transaction_id?: string;
    }) => api.post<AccountFlow>('/api/flows', data),

    confirmFlow: (id: string) => api.put<AccountFlow>(`/api/flows/${id}`, { confirmed: true }),

    dismissFlow: (id: string) => api.put<AccountFlow>(`/api/flows/${id}`, { dismissed: true }),

    deleteFlow: (id: string) => api.del<void>(`/api/flows/${id}`),

    getSankeyData: (month: string) => api.get<SankeyData>(`/api/flows/sankey?month=${month}`),

    getOutflowRanking: (month: string) =>
      api.get<OutflowRank[]>(`/api/flows/outflow-ranking?month=${month}`),

    getBalanceImpact: (month: string, accountId: string) =>
      api.get<WaterfallData>(`/api/flows/balance-impact?month=${month}&account_id=${accountId}`),

    listFlowGroups: () => api.get<FlowGroup[]>('/api/flow-groups'),

    createFlowGroup: (data: { source_account_id: string; destination_account_id: string }) =>
      api.post<FlowGroup>('/api/flow-groups', data),

    updateFlowGroup: (
      id: string,
      data: Partial<{ source_account_id: string; destination_account_id: string }>,
    ) => api.put<FlowGroup>(`/api/flow-groups/${id}`, data),

    deleteFlowGroup: (id: string) => api.del<void>(`/api/flow-groups/${id}`),
  };
}
