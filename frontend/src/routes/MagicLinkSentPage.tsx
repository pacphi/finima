import { useLocation, useNavigate } from 'react-router';
import { useState } from 'react';
import { requestMagicLink } from '@/api/auth';

export function MagicLinkSentPage() {
  const location = useLocation();
  const navigate = useNavigate();
  const email = (location.state as { email?: string } | null)?.email ?? '';
  const [resending, setResending] = useState(false);
  const [resent, setResent] = useState(false);

  const handleResend = async () => {
    if (!email) {
      navigate('/auth/signin');
      return;
    }
    setResending(true);
    try {
      await requestMagicLink(email);
      setResent(true);
    } catch {
      // Silently handle — user can try again
    } finally {
      setResending(false);
    }
  };

  return (
    <div className="min-h-screen flex items-center justify-center bg-[var(--color-bg)] relative overflow-hidden">
      {/* Background glow */}
      <div className="absolute top-1/3 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[400px] h-[400px] bg-[var(--color-primary-glow)] rounded-full blur-[100px] pointer-events-none" />

      <div className="w-full max-w-sm p-8 text-center relative z-10">
        {/* Email icon */}
        <div className="mx-auto mb-6 w-16 h-16 rounded-2xl bg-[var(--color-primary-subtle)] border border-[var(--color-primary-muted)] flex items-center justify-center">
          <svg
            className="w-8 h-8 text-[var(--color-primary)]"
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
            strokeWidth={1.5}
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              d="M21.75 6.75v10.5a2.25 2.25 0 0 1-2.25 2.25h-15a2.25 2.25 0 0 1-2.25-2.25V6.75m19.5 0A2.25 2.25 0 0 0 19.5 4.5h-15a2.25 2.25 0 0 0-2.25 2.25m19.5 0v.243a2.25 2.25 0 0 1-1.07 1.916l-7.5 4.615a2.25 2.25 0 0 1-2.36 0L3.32 8.91a2.25 2.25 0 0 1-1.07-1.916V6.75"
            />
          </svg>
        </div>

        <h1 className="text-2xl font-bold text-[var(--color-text)] mb-3 tracking-tight">
          Check your email
        </h1>
        <p className="text-[var(--color-text-secondary)] text-sm mb-1">We sent a sign-in link to</p>
        <p className="font-semibold text-[var(--color-text)] mb-4">
          {email || 'your email address'}
        </p>

        <div className="rounded-2xl border border-[var(--color-border)] bg-[var(--color-card)] backdrop-blur-md p-4 mb-6">
          <p className="text-xs text-[var(--color-text-secondary)]">
            Click the link in the email to sign in. It expires in 15 minutes.
          </p>
        </div>

        <button
          onClick={handleResend}
          disabled={resending || resent}
          className="py-2.5 px-6 border border-[var(--color-border)] rounded-xl
            text-[var(--color-text-secondary)] hover:text-[var(--color-text)] hover:bg-[var(--color-primary-subtle)]
            transition-all duration-200 disabled:opacity-50 text-sm font-medium"
        >
          {resent ? 'Link resent!' : resending ? 'Resending...' : "Didn't receive it? Resend"}
        </button>
      </div>
    </div>
  );
}
