import { create } from 'zustand';

export interface DashboardWidget {
  id: string;
  label: string;
  visible: boolean;
}

const DEFAULT_WIDGETS: DashboardWidget[] = [
  { id: 'net-worth', label: 'Net Worth', visible: true },
  { id: 'cashflow', label: 'Cash Flow', visible: true },
  { id: 'spending', label: 'Spending by Category', visible: true },
  { id: 'budget', label: 'Budget vs Actual', visible: true },
  { id: 'goals', label: 'Savings Goals', visible: true },
  { id: 'upcoming', label: 'Upcoming Bills', visible: true },
  { id: 'health', label: 'Financial Health', visible: true },
];

interface PrefsState {
  currency: string;
  dateFormat: string;
  fiscalMonth: number;
  defaultChartType: string;
  dashboardWidgets: DashboardWidget[];
  loaded: boolean;

  updatePref: (key: string, value: unknown) => void;
  toggleWidget: (widgetId: string) => void;
  resetDashboardLayout: () => void;
  loadFromApi: (apiFn: <T>(path: string) => Promise<T>) => Promise<void>;
  saveToApi: (apiFn: <T>(path: string, body?: unknown) => Promise<T>) => Promise<void>;
}

const STORAGE_KEY = 'finima-prefs';

function loadFromStorage(): Partial<PrefsState> {
  if (typeof localStorage === 'undefined') return {};
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (stored) return JSON.parse(stored);
  } catch {
    // Ignore parse errors.
  }
  return {};
}

function saveToStorage(state: Partial<PrefsState>) {
  if (typeof localStorage === 'undefined') return;
  localStorage.setItem(
    STORAGE_KEY,
    JSON.stringify({
      currency: state.currency,
      dateFormat: state.dateFormat,
      fiscalMonth: state.fiscalMonth,
      defaultChartType: state.defaultChartType,
      dashboardWidgets: state.dashboardWidgets,
    }),
  );
}

export const usePrefsStore = create<PrefsState>()((set, get) => {
  const stored = loadFromStorage();

  return {
    currency: (stored.currency as string) ?? 'USD',
    dateFormat: (stored.dateFormat as string) ?? 'MM/DD/YYYY',
    fiscalMonth: (stored.fiscalMonth as number) ?? 1,
    defaultChartType: (stored.defaultChartType as string) ?? 'line',
    dashboardWidgets: (stored.dashboardWidgets as DashboardWidget[]) ?? [...DEFAULT_WIDGETS],
    loaded: false,

    updatePref: (key, value) => {
      set((s) => {
        const next = { ...s, [key]: value };
        saveToStorage(next);
        return next;
      });
    },

    toggleWidget: (widgetId) => {
      set((s) => {
        const widgets = s.dashboardWidgets.map((w) =>
          w.id === widgetId ? { ...w, visible: !w.visible } : w,
        );
        const next = { ...s, dashboardWidgets: widgets };
        saveToStorage(next);
        return { dashboardWidgets: widgets };
      });
    },

    resetDashboardLayout: () => {
      const widgets = [...DEFAULT_WIDGETS];
      set({ dashboardWidgets: widgets });
      saveToStorage({ ...get(), dashboardWidgets: widgets });
    },

    loadFromApi: async (apiFn) => {
      try {
        const profile = await apiFn<{ preferences: Record<string, unknown> }>('/api/users/me');
        if (profile.preferences && typeof profile.preferences === 'object') {
          const p = profile.preferences;
          set({
            currency: (p.currency as string) ?? get().currency,
            dateFormat: (p.dateFormat as string) ?? get().dateFormat,
            fiscalMonth: (p.fiscalMonth as number) ?? get().fiscalMonth,
            defaultChartType: (p.defaultChartType as string) ?? get().defaultChartType,
            dashboardWidgets: (p.dashboardWidgets as DashboardWidget[]) ?? get().dashboardWidgets,
            loaded: true,
          });
          saveToStorage(get());
        }
      } catch {
        // Fall back to local storage values.
        set({ loaded: true });
      }
    },

    saveToApi: async (apiFn) => {
      const s = get();
      try {
        await apiFn('/api/users/me/preferences', {
          currency: s.currency,
          dateFormat: s.dateFormat,
          fiscalMonth: s.fiscalMonth,
          defaultChartType: s.defaultChartType,
          dashboardWidgets: s.dashboardWidgets,
        });
      } catch (err) {
        console.error('Failed to save preferences to API:', err);
        throw err;
      }
    },
  };
});
