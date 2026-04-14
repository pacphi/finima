import { useAuthStore } from '@/stores/authStore';
import { useNavigate } from 'react-router-dom';
import { logout as logoutApi } from '@/api/auth';

export function Header() {
  const user = useAuthStore((s) => s.user);
  const logoutStore = useAuthStore((s) => s.logout);
  const navigate = useNavigate();

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
      <h1 className="text-lg font-semibold text-[var(--color-text)] md:hidden">
        {/* Hidden on desktop since sidebar shows the name */}
      </h1>
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
