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

  return (
    <header
      role="banner"
      className="h-16 bg-[var(--color-bg)] border-b border-[var(--color-border)] flex items-center justify-between px-6"
    >
      <h1 className="text-lg font-semibold text-[var(--color-text)] md:hidden">
        {/* Hidden on desktop since sidebar shows the name */}
      </h1>
      <div className="flex-1" />
      <div className="flex items-center gap-4">
        {user && (
          <span className="text-sm text-[var(--color-text-secondary)]">{user.displayName}</span>
        )}
        <button
          onClick={handleLogout}
          aria-label="Log out of your account"
          className="text-sm text-[var(--color-text-secondary)] hover:text-[var(--color-text)]
            transition-colors"
        >
          Logout
        </button>
      </div>
    </header>
  );
}
