import { create } from 'zustand';

export interface User {
  id: string;
  email: string;
  displayName: string;
}

interface AuthState {
  user: User | null;
  accessToken: string | null;
  refreshToken: string | null;
  isAuthenticated: boolean;
  login: (user: User, tokens: { accessToken: string; refreshToken: string }) => void;
  logout: () => void;
  setTokens: (accessToken: string, refreshToken: string) => void;
}

const STORAGE_KEY = 'finima-auth';

function loadFromSession(): {
  user: User | null;
  accessToken: string | null;
  refreshToken: string | null;
} {
  if (typeof sessionStorage === 'undefined') {
    return { user: null, accessToken: null, refreshToken: null };
  }
  try {
    const stored = sessionStorage.getItem(STORAGE_KEY);
    if (stored) {
      const parsed = JSON.parse(stored);
      return {
        user: parsed.user ?? null,
        accessToken: parsed.accessToken ?? null,
        refreshToken: parsed.refreshToken ?? null,
      };
    }
  } catch {
    // Ignore parse errors.
  }
  return { user: null, accessToken: null, refreshToken: null };
}

function saveToSession(user: User | null, accessToken: string | null, refreshToken: string | null) {
  if (typeof sessionStorage === 'undefined') return;
  sessionStorage.setItem(STORAGE_KEY, JSON.stringify({ user, accessToken, refreshToken }));
}

function clearSession() {
  if (typeof sessionStorage === 'undefined') return;
  sessionStorage.removeItem(STORAGE_KEY);
}

export const useAuthStore = create<AuthState>()((set) => {
  const hydrated = loadFromSession();

  return {
    user: hydrated.user,
    accessToken: hydrated.accessToken,
    refreshToken: hydrated.refreshToken,
    isAuthenticated: hydrated.accessToken !== null,

    login: (user, tokens) => {
      saveToSession(user, tokens.accessToken, tokens.refreshToken);
      set({
        user,
        accessToken: tokens.accessToken,
        refreshToken: tokens.refreshToken,
        isAuthenticated: true,
      });
    },

    logout: () => {
      clearSession();
      set({
        user: null,
        accessToken: null,
        refreshToken: null,
        isAuthenticated: false,
      });
    },

    setTokens: (accessToken, refreshToken) => {
      const currentState = useAuthStore.getState();
      saveToSession(currentState.user, accessToken, refreshToken);
      set({ accessToken, refreshToken });
    },
  };
});
