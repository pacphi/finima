import { NavLink } from 'react-router-dom';
import { useState, useEffect, useRef, useCallback } from 'react';
import { useHealthStore, type LlmStatus } from '@/stores/healthStore';

interface NavItem {
  to: string;
  label: string;
  /** When true the item is dimmed and unclickable while the LLM is not ready. */
  requiresLlm?: boolean;
}

const navItems: NavItem[] = [
  { to: '/dashboard', label: 'Dashboard' },
  { to: '/accounts', label: 'Accounts' },
  { to: '/transactions', label: 'Transactions' },
  { to: '/recurring', label: 'Recurring' },
  { to: '/flows', label: 'Money Flow' },
  { to: '/budget', label: 'Budget' },
  { to: '/goals', label: 'Goals' },
  { to: '/news', label: 'News' },
  { to: '/settings', label: 'Settings' },
];

export function Sidebar() {
  const [mobileOpen, setMobileOpen] = useState(false);
  const sidebarRef = useRef<HTMLElement>(null);
  const navRef = useRef<HTMLElement>(null);
  const llmStatus = useHealthStore((s) => s.llmStatus);
  const llmReady = llmStatus === 'ready';

  // Close on outside click.
  useEffect(() => {
    if (!mobileOpen) return;

    const handleClick = (e: MouseEvent) => {
      if (sidebarRef.current && !sidebarRef.current.contains(e.target as Node)) {
        setMobileOpen(false);
      }
    };

    document.addEventListener('mousedown', handleClick);
    return () => document.removeEventListener('mousedown', handleClick);
  }, [mobileOpen]);

  // Close on Escape key.
  useEffect(() => {
    if (!mobileOpen) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setMobileOpen(false);
    };

    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [mobileOpen]);

  // Arrow key navigation within nav items
  const handleNavKeyDown = useCallback((e: React.KeyboardEvent<HTMLElement>) => {
    if (e.key !== 'ArrowDown' && e.key !== 'ArrowUp') return;
    e.preventDefault();

    const nav = navRef.current;
    if (!nav) return;

    const links = Array.from(nav.querySelectorAll<HTMLAnchorElement>('a[role="menuitem"]'));
    const currentIndex = links.findIndex((link) => link === document.activeElement);

    let nextIndex: number;
    if (e.key === 'ArrowDown') {
      nextIndex = currentIndex < links.length - 1 ? currentIndex + 1 : 0;
    } else {
      nextIndex = currentIndex > 0 ? currentIndex - 1 : links.length - 1;
    }

    links[nextIndex]?.focus();
  }, []);

  return (
    <>
      {/* Mobile hamburger button */}
      <button
        className="md:hidden fixed top-4 left-4 z-50 p-2 bg-[var(--sidebar-bg)] text-[var(--sidebar-text)] rounded-lg shadow-lg"
        onClick={() => setMobileOpen(!mobileOpen)}
        aria-label={mobileOpen ? 'Close navigation menu' : 'Open navigation menu'}
        aria-expanded={mobileOpen}
        aria-controls="sidebar-nav"
      >
        <svg
          className="w-6 h-6"
          fill="none"
          stroke="currentColor"
          viewBox="0 0 24 24"
          aria-hidden="true"
        >
          {mobileOpen ? (
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={2}
              d="M6 18L18 6M6 6l12 12"
            />
          ) : (
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={2}
              d="M4 6h16M4 12h16M4 18h16"
            />
          )}
        </svg>
      </button>

      {/* Overlay for mobile */}
      {mobileOpen && (
        <div
          className="md:hidden fixed inset-0 bg-black/50 z-30 transition-opacity"
          aria-hidden="true"
        />
      )}

      {/* Sidebar */}
      <aside
        ref={sidebarRef}
        id="sidebar-nav"
        aria-label="Main navigation"
        className={`
          fixed md:static z-40 top-0 left-0 h-full w-64
          bg-[var(--sidebar-bg)] text-[var(--sidebar-text)]
          flex flex-col transition-transform duration-200 ease-in-out
          ${mobileOpen ? 'translate-x-0' : '-translate-x-full md:translate-x-0'}
        `}
      >
        <div className="p-6">
          <h2 className="text-xl font-bold">Finima</h2>
        </div>

        <nav
          ref={navRef}
          role="navigation"
          aria-label="Primary"
          className="flex-1 px-4 space-y-1 overflow-y-auto"
          onKeyDown={handleNavKeyDown}
        >
          <ul role="menubar" aria-orientation="vertical" className="space-y-1">
            {navItems.map((item) => {
              const disabled = item.requiresLlm && !llmReady;
              if (disabled) {
                return (
                  <li key={item.to} role="none">
                    <span
                      role="menuitem"
                      aria-disabled="true"
                      title="Waiting for AI model to load"
                      className="block px-4 py-2.5 rounded-lg text-sm text-slate-500 cursor-not-allowed select-none"
                    >
                      {item.label}
                    </span>
                  </li>
                );
              }
              return (
                <li key={item.to} role="none">
                  <NavLink
                    to={item.to}
                    role="menuitem"
                    onClick={() => setMobileOpen(false)}
                    className={({ isActive }) =>
                      `block px-4 py-2.5 rounded-lg text-sm transition-colors ${
                        isActive
                          ? 'bg-[var(--sidebar-active)] text-white font-medium'
                          : 'text-slate-300 hover:bg-[var(--sidebar-hover)] hover:text-white'
                      }`
                    }
                  >
                    {item.label}
                  </NavLink>
                </li>
              );
            })}
          </ul>
        </nav>

        <LlmStatusIndicator status={llmStatus} />
      </aside>
    </>
  );
}

// ── LLM status indicator ────────────────────────────────────────────

const STATUS_CONFIG: Record<LlmStatus, { dot: string; label: string; animate?: boolean }> = {
  loading: { dot: 'bg-amber-400', label: 'AI model loading', animate: true },
  ready: { dot: 'bg-emerald-400', label: 'AI model ready' },
  failed: { dot: 'bg-red-400', label: 'AI model failed to load' },
  unknown: { dot: 'bg-slate-400', label: 'Checking AI status', animate: true },
};

function LlmStatusIndicator({ status }: { status: LlmStatus }) {
  const cfg = STATUS_CONFIG[status];
  return (
    <div className="px-4 py-3 border-t border-white/10" aria-live="polite">
      <div className="flex items-center gap-2 text-xs text-slate-400">
        <span
          className={`inline-block w-2 h-2 rounded-full ${cfg.dot} ${cfg.animate ? 'animate-pulse' : ''}`}
          aria-hidden="true"
        />
        <span>{cfg.label}</span>
      </div>
    </div>
  );
}
