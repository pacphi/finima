import { useLocation, useNavigate } from 'react-router-dom';
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
    <div className="min-h-screen flex items-center justify-center bg-[var(--color-surface)]">
      <div className="w-full max-w-md p-8 text-center">
        <div className="mb-6 text-5xl">&#9993;</div>
        <h1 className="text-2xl font-bold text-[var(--color-text)] mb-4">Check your email</h1>
        <p className="text-[var(--color-text-secondary)] mb-2">We sent a sign-in link to</p>
        <p className="font-medium text-[var(--color-text)] mb-4">{email || 'your email address'}</p>
        <p className="text-sm text-[var(--color-text-secondary)] mb-8">
          Click the link to sign in. It expires in 15 minutes.
        </p>

        <button
          onClick={handleResend}
          disabled={resending || resent}
          className="py-2 px-6 border border-[var(--color-border)] rounded-lg
            text-[var(--color-text-secondary)] hover:bg-[var(--color-surface)]
            transition-colors disabled:opacity-50"
        >
          {resent ? 'Link resent!' : resending ? 'Resending...' : "Didn't receive it? Resend"}
        </button>
      </div>
    </div>
  );
}
