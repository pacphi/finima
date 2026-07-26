import { useAuthStore } from '@/stores/authStore';
import { usePortfolioStore } from '@/stores/portfolioStore';
import { useNavigate } from 'react-router';
import { logout as logoutApi } from '@/api/auth';

export function Header() {
  const user = useAuthStore((s) => s.user);
  const logoutStore = useAuthStore((s) => s.logout);
  const navigate = useNavigate();

  const portfolios = usePortfolioStore((s) => s.portfolios);
  const activePortfolioId = usePortfolioStore((s) => s.activePortfolioId);
  const selectPortfolio = usePortfolioStore((s) => s.selectPortfolio);

  const handleLogout = async () => {
    try {
      await logoutApi();
    } catch {
      // Proceed with local logout even if API call fails
    }
    logoutStore();
    navigate('/auth/signin');
  };

  const initials = user?.displayName
    ? user.displayName
        .split(' ')
        .map((n) => n[0])
        .join('')
        .toUpperCase()
        .slice(0, 2)
    : '?';

  return (
    <header
      role="banner"
      className="h-16 bg-[var(--color-bg)]/80 backdrop-blur-md border-b border-[var(--color-border)] flex items-center justify-between px-6"
    >
      <div className="flex items-center gap-2">
        {portfolios.length > 0 && (
          <select
            value={activePortfolioId ?? ''}
            onChange={(e) => selectPortfolio(e.target.value)}
            aria-label="Select portfolio"
            className="text-sm font-medium bg-[var(--color-input-bg)] text-[var(--color-text)] border border-[var(--color-input-border)] rounded-lg px-3 py-1.5 focus:border-[var(--color-primary)] focus:ring-1 focus:ring-[var(--color-primary)] outline-none"
          >
            {portfolios.map((p) => (
              <option key={p.id} value={p.id}>
                {p.name}
              </option>
            ))}
          </select>
        )}
        <button
          onClick={() => navigate('/portfolios')}
          aria-label="Manage portfolios"
          title="Manage portfolios"
          className="p-1.5 text-[var(--color-text-secondary)] hover:text-[var(--color-text)] hover:bg-[var(--color-border)] rounded-lg transition-colors"
        >
          <svg
            xmlns="http://www.w3.org/2000/svg"
            width="16"
            height="16"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
          >
            <path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z" />
            <circle cx="12" cy="12" r="3" />
          </svg>
        </button>
      </div>
      <div className="flex-1" />
      <div className="flex items-center gap-3">
        {user && (
          <div className="flex items-center gap-3">
            <div className="w-8 h-8 rounded-full bg-gradient-to-br from-[var(--color-primary)] to-[var(--color-primary-hover)] flex items-center justify-center text-xs font-semibold text-white shadow-sm">
              {initials}
            </div>
            <span className="text-sm font-medium text-[var(--color-text)] hidden sm:inline">
              {user.displayName}
            </span>
          </div>
        )}
        <div className="w-px h-6 bg-[var(--color-border)] mx-1" />
        <button
          onClick={handleLogout}
          aria-label="Log out of your account"
          className="text-sm text-[var(--color-text-secondary)] hover:text-[var(--color-text)]
            transition-colors px-2 py-1.5 rounded-lg hover:bg-[var(--color-border)]"
        >
          Logout
        </button>
      </div>
    </header>
  );
}
