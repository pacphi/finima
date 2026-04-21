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

// Persist the active portfolio id in localStorage (not sessionStorage) so
// the selection survives full reloads and tab restores. Matches the
// project convention for user-visible preferences (themeStore,
// prefsStore). The persisted id is revalidated against the portfolio
// list at hydrate time, so a stale id for a portfolio the user no longer
// owns silently falls back to the first portfolio.
const STORAGE_KEY = 'finima-active-portfolio';

function loadStoredPortfolioId(): string | null {
  if (typeof localStorage === 'undefined') return null;
  try {
    return localStorage.getItem(STORAGE_KEY);
  } catch {
    return null;
  }
}

function storeActivePortfolioId(id: string | null) {
  if (typeof localStorage === 'undefined') return;
  try {
    if (id) {
      localStorage.setItem(STORAGE_KEY, id);
    } else {
      localStorage.removeItem(STORAGE_KEY);
    }
  } catch {
    // storage quota or privacy mode — ignore
  }
}

export const usePortfolioStore = create<PortfolioState>()((set) => ({
  portfolios: [],
  activePortfolioId: loadStoredPortfolioId(),
  accounts: [],
  loading: false,
  error: null,

  setPortfolios: (portfolios) =>
    set((state) => {
      // Prefer an already-selected id if it still exists in the new list;
      // otherwise fall back to a persisted id if it matches a portfolio;
      // otherwise default to the first portfolio.
      const ids = new Set(portfolios.map((p) => p.id));
      const candidate =
        (state.activePortfolioId && ids.has(state.activePortfolioId)
          ? state.activePortfolioId
          : null) ??
        (loadStoredPortfolioId() && ids.has(loadStoredPortfolioId() as string)
          ? loadStoredPortfolioId()
          : null) ??
        (portfolios.length > 0 ? (portfolios[0]?.id ?? null) : null);
      storeActivePortfolioId(candidate);
      return { portfolios, activePortfolioId: candidate };
    }),

  selectPortfolio: (id) => {
    storeActivePortfolioId(id);
    set({ activePortfolioId: id, accounts: [] });
  },

  setAccounts: (accounts) => set({ accounts }),

  addPortfolio: (portfolio) =>
    set((state) => {
      const nextActive = state.activePortfolioId ?? portfolio.id;
      if (nextActive !== state.activePortfolioId) {
        storeActivePortfolioId(nextActive);
      }
      return {
        portfolios: [...state.portfolios, portfolio],
        activePortfolioId: nextActive,
      };
    }),

  addAccount: (account) => set((state) => ({ accounts: [...state.accounts, account] })),

  removeAccount: (id) => set((state) => ({ accounts: state.accounts.filter((a) => a.id !== id) })),

  updateAccount: (id, data) =>
    set((state) => ({
      accounts: state.accounts.map((a) => (a.id === id ? { ...a, ...data } : a)),
    })),

  setLoading: (loading) => set({ loading }),
  setError: (error) => set({ error }),
}));
