import { useEffect } from 'react';
import { Outlet } from 'react-router-dom';
import { Sidebar } from './Sidebar';
import { Header } from './Header';
import { useThemeStore } from '@/stores/themeStore';
import { useConfigStore } from '@/stores/configStore';
import { useHealthStore } from '@/stores/healthStore';

export function AppLayout() {
  const initTheme = useThemeStore((s) => s.initTheme);
  const apiBaseUrl = useConfigStore((s) => s.apiBaseUrl);
  const startPolling = useHealthStore((s) => s.startPolling);

  // Initialize theme on first mount.
  useEffect(() => {
    initTheme();
  }, [initTheme]);

  // Poll /health to track LLM backend readiness.
  useEffect(() => {
    return startPolling(apiBaseUrl);
  }, [apiBaseUrl, startPolling]);

  return (
    <div className="flex h-screen overflow-hidden bg-[var(--color-bg)]">
      {/* Skip to main content link for keyboard users */}
      <a
        href="#main-content"
        className="sr-only focus:not-sr-only focus:absolute focus:z-[100] focus:top-2 focus:left-2 focus:px-4 focus:py-2 focus:bg-[var(--color-primary)] focus:text-white focus:rounded-xl focus:text-sm focus:font-medium"
      >
        Skip to main content
      </a>
      <Sidebar />
      <div className="flex-1 flex flex-col overflow-hidden min-w-0">
        <Header />
        <main
          id="main-content"
          role="main"
          className="flex-1 overflow-auto bg-[var(--color-surface)]"
          tabIndex={-1}
        >
          <div className="min-w-0">
            <Outlet />
          </div>
        </main>
      </div>
    </div>
  );
}
