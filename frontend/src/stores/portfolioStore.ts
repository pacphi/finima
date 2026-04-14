import { create } from 'zustand';
import type { Portfolio, Account } from '@/types/models';

interface PortfolioState {
  portfolios: Portfolio[];
  activePortfolioId: string | null;
  accounts: Account[];
  loading: boolean;
  error: string | null;

  setPortfolios: (portfolios: Portfolio[]) => void;
  selectPortfolio: (id: string) => void;
  setAccounts: (accounts: Account[]) => void;
  addPortfolio: (portfolio: Portfolio) => void;
  addAccount: (account: Account) => void;
  removeAccount: (id: string) => void;
  updateAccount: (id: string, data: Partial<Account>) => void;
  setLoading: (loading: boolean) => void;
  setError: (error: string | null) => void;
}

export const usePortfolioStore = create<PortfolioState>()((set) => ({
  portfolios: [],
  activePortfolioId: null,
  accounts: [],
  loading: false,
  error: null,

  setPortfolios: (portfolios) =>
    set({
      portfolios,
      activePortfolioId: portfolios.length > 0 ? (portfolios[0]?.id ?? null) : null,
    }),

  selectPortfolio: (id) => set({ activePortfolioId: id, accounts: [] }),

  setAccounts: (accounts) => set({ accounts }),

  addPortfolio: (portfolio) =>
    set((state) => ({
      portfolios: [...state.portfolios, portfolio],
      activePortfolioId: state.activePortfolioId ?? portfolio.id,
    })),

  addAccount: (account) => set((state) => ({ accounts: [...state.accounts, account] })),

  removeAccount: (id) => set((state) => ({ accounts: state.accounts.filter((a) => a.id !== id) })),

  updateAccount: (id, data) =>
    set((state) => ({
      accounts: state.accounts.map((a) => (a.id === id ? { ...a, ...data } : a)),
    })),

  setLoading: (loading) => set({ loading }),
  setError: (error) => set({ error }),
}));
