import { useEffect, useState } from 'react';
import { useSearchParams, useNavigate } from 'react-router';
import { verifyToken } from '@/api/auth';
import { useAuthStore } from '@/stores/authStore';

export function VerifyPage() {
  const [searchParams] = useSearchParams();
  const navigate = useNavigate();
  const login = useAuthStore((s) => s.login);
  const [asyncError, setAsyncError] = useState<string | null>(null);

  const token = searchParams.get('token');
  const email = searchParams.get('email');
  const missingParams = !token || !email;

  useEffect(() => {
    if (missingParams) return;

    let cancelled = false;

    async function verify() {
      try {
        const result = await verifyToken(token!, email!);

        if (cancelled) return;

        login(
          {
            id: result.user.id,
            email: result.user.email,
            displayName: result.user.display_name,
          },
          {
            accessToken: result.access_token,
            refreshToken: result.refresh_token,
          },
        );

        if (result.is_new_user) {
          navigate('/onboarding', { replace: true });
        } else {
          navigate('/dashboard', { replace: true });
        }
      } catch (err) {
        if (!cancelled) {
          setAsyncError(err instanceof Error ? err.message : 'Verification failed');
        }
      }
    }

    void verify();

    return () => {
      cancelled = true;
    };
  }, [missingParams, token, email, navigate, login]);

  const error = missingParams ? 'Invalid verification link. Missing token or email.' : asyncError;

  if (error) {
    return (
      <div className="min-h-screen flex items-center justify-center bg-[var(--color-surface)]">
        <div className="w-full max-w-md p-8 text-center">
          <h1 className="text-2xl font-bold text-red-600 mb-4">Verification Failed</h1>
          <p className="text-[var(--color-text-secondary)] mb-6">{error}</p>
          <a href="/auth/signin" className="text-[var(--color-primary)] hover:underline">
            Back to sign in
          </a>
        </div>
      </div>
    );
  }

  return (
    <div className="min-h-screen flex items-center justify-center bg-[var(--color-surface)]">
      <div className="w-full max-w-md p-8 text-center">
        <p className="text-[var(--color-text-secondary)]">Verifying your sign-in link...</p>
      </div>
    </div>
  );
}
