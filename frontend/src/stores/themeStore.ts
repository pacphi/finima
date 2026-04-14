import { create } from 'zustand';

export type ThemeMode = 'light' | 'dark' | 'system';

interface ThemeState {
  mode: ThemeMode;
  accentColor: string;
  setMode: (mode: ThemeMode) => void;
  setAccentColor: (color: string) => void;
  initTheme: () => void;
}

const STORAGE_KEY = 'finima-theme';
const DEFAULT_ACCENT = '#10b981';

function getSystemPreference(): 'light' | 'dark' {
  if (typeof window === 'undefined') return 'light';
  return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
}

function applyDarkClass(isDark: boolean) {
  if (typeof document === 'undefined') return;
  if (isDark) {
    document.documentElement.classList.add('dark');
  } else {
    document.documentElement.classList.remove('dark');
  }
}

function hexToRgb(hex: string): { r: number; g: number; b: number } {
  const num = parseInt(hex.replace('#', ''), 16);
  return {
    r: (num >> 16) & 0xff,
    g: (num >> 8) & 0xff,
    b: num & 0xff,
  };
}

function applyAccentColor(color: string) {
  if (typeof document === 'undefined') return;
  const root = document.documentElement;
  const { r, g, b } = hexToRgb(color);

  root.style.setProperty('--color-primary', color);
  root.style.setProperty('--color-primary-hover', adjustBrightness(color, -15));
  root.style.setProperty('--color-primary-glow', `rgba(${r}, ${g}, ${b}, 0.25)`);
  root.style.setProperty('--color-primary-subtle', `rgba(${r}, ${g}, ${b}, 0.08)`);
  root.style.setProperty('--color-primary-muted', `rgba(${r}, ${g}, ${b}, 0.4)`);

  // Badge colors adapt to light/dark mode
  const isDark = root.classList.contains('dark');
  if (isDark) {
    root.style.setProperty('--color-primary-badge-bg', `rgba(${r}, ${g}, ${b}, 0.15)`);
    root.style.setProperty('--color-primary-badge-text', adjustBrightness(color, 40));
  } else {
    root.style.setProperty('--color-primary-badge-bg', adjustBrightness(color, 80));
    root.style.setProperty('--color-primary-badge-text', adjustBrightness(color, -40));
  }
}

function adjustBrightness(hex: string, percent: number): string {
  const num = parseInt(hex.replace('#', ''), 16);
  const r = Math.min(255, Math.max(0, ((num >> 16) & 0xff) + percent));
  const g = Math.min(255, Math.max(0, ((num >> 8) & 0xff) + percent));
  const b = Math.min(255, Math.max(0, (num & 0xff) + percent));
  return `#${((r << 16) | (g << 8) | b).toString(16).padStart(6, '0')}`;
}

function loadFromStorage(): { mode: ThemeMode; accentColor: string } {
  if (typeof localStorage === 'undefined') {
    return { mode: 'system', accentColor: DEFAULT_ACCENT };
  }
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (stored) {
      const parsed = JSON.parse(stored);
      return {
        mode: parsed.mode ?? 'system',
        accentColor: parsed.accentColor ?? DEFAULT_ACCENT,
      };
    }
  } catch {
    // Ignore parse errors.
  }
  return { mode: 'system', accentColor: DEFAULT_ACCENT };
}

function saveToStorage(mode: ThemeMode, accentColor: string) {
  if (typeof localStorage === 'undefined') return;
  localStorage.setItem(STORAGE_KEY, JSON.stringify({ mode, accentColor }));
}

function resolveEffectiveMode(mode: ThemeMode): 'light' | 'dark' {
  if (mode === 'system') return getSystemPreference();
  return mode;
}

export const useThemeStore = create<ThemeState>()((set, get) => ({
  mode: 'system',
  accentColor: DEFAULT_ACCENT,

  setMode: (mode) => {
    const accentColor = get().accentColor;
    applyDarkClass(resolveEffectiveMode(mode) === 'dark');
    applyAccentColor(accentColor);
    saveToStorage(mode, accentColor);
    set({ mode });
  },

  setAccentColor: (color) => {
    const mode = get().mode;
    applyAccentColor(color);
    saveToStorage(mode, color);
    set({ accentColor: color });
  },

  initTheme: () => {
    const { mode, accentColor } = loadFromStorage();
    applyDarkClass(resolveEffectiveMode(mode) === 'dark');
    applyAccentColor(accentColor);
    set({ mode, accentColor });

    // Listen for system preference changes when mode is "system".
    if (typeof window !== 'undefined') {
      window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', () => {
        const current = get().mode;
        if (current === 'system') {
          applyDarkClass(getSystemPreference() === 'dark');
          applyAccentColor(get().accentColor);
        }
      });
    }
  },
}));
