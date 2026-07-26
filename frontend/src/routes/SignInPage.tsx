import { useForm } from 'react-hook-form';
import { zodResolver } from '@hookform/resolvers/zod';
import { z } from 'zod';
import { useNavigate } from 'react-router';
import { useState } from 'react';
import { requestMagicLink } from '@/api/auth';

const signInSchema = z.object({
  email: z.string().email('Please enter a valid email address'),
});

type SignInForm = z.infer<typeof signInSchema>;

export function SignInPage() {
  const navigate = useNavigate();
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const {
    register,
    handleSubmit,
    formState: { errors },
  } = useForm<SignInForm>({
    resolver: zodResolver(signInSchema),
  });

  const onSubmit = async (data: SignInForm) => {
    setError(null);
    setLoading(true);
    try {
      await requestMagicLink(data.email);
      navigate('/auth/magic-link-sent', { state: { email: data.email } });
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Something went wrong');
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="min-h-screen flex items-center justify-center bg-[var(--color-bg)] relative overflow-hidden">
      {/* Background glow effects */}
      <div className="absolute top-1/4 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[500px] h-[500px] bg-[var(--color-primary-glow)] rounded-full blur-[120px] pointer-events-none" />
      <div className="absolute bottom-0 right-0 w-[300px] h-[300px] bg-[var(--color-primary-subtle)] rounded-full blur-[80px] pointer-events-none" />

      <div className="w-full max-w-sm p-8 relative z-10">
        {/* Logo */}
        <div className="text-center mb-10">
          <div className="mx-auto mb-5 w-14 h-14 rounded-2xl bg-gradient-to-br from-[var(--color-primary)] to-[var(--color-primary-hover)] flex items-center justify-center shadow-lg shadow-[var(--color-primary-glow)]">
            <svg
              className="w-7 h-7 text-white"
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor"
              strokeWidth={2}
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                d="M12 6v12m-3-2.818.879.659c1.171.879 3.07.879 4.242 0 1.172-.879 1.172-2.303 0-3.182C13.536 12.219 12.768 12 12 12c-.725 0-1.45-.22-2.003-.659-1.106-.879-1.106-2.303 0-3.182s2.9-.879 4.006 0l.415.33M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0Z"
              />
            </svg>
          </div>
          <h1 className="text-3xl font-bold text-[var(--color-text)] tracking-tight">Finima</h1>
          <p className="mt-2 text-sm text-[var(--color-text-secondary)]">
            Your finances, your intelligence.
          </p>
        </div>

        {/* Sign-in card */}
        <div className="rounded-2xl border border-[var(--color-border)] bg-[var(--color-card)] backdrop-blur-md p-6">
          <form onSubmit={handleSubmit(onSubmit)} className="space-y-4" noValidate>
            <div>
              <label
                htmlFor="signin-email"
                className="block text-xs font-medium text-[var(--color-text-secondary)] mb-2 uppercase tracking-wider"
              >
                Email address
              </label>
              <input
                id="signin-email"
                type="email"
                placeholder="you@example.com"
                aria-required="true"
                aria-invalid={!!errors.email}
                aria-describedby={errors.email ? 'email-error' : undefined}
                {...register('email')}
                onKeyDown={(e) => {
                  if (e.key === 'Enter' && !loading) {
                    e.preventDefault();
                    void handleSubmit(onSubmit)();
                  }
                }}
                className="w-full px-4 py-3 border border-[var(--color-input-border)] rounded-xl
                  focus:outline-none focus:ring-2 focus:ring-[var(--color-primary-glow)] focus:border-[var(--color-primary)]
                  bg-[var(--color-input-bg)] text-[var(--color-text)] placeholder-[var(--color-text-secondary)] text-sm
                  transition-all duration-200"
              />
              {errors.email && (
                <p id="email-error" className="mt-2 text-xs text-[var(--color-error)]" role="alert">
                  {errors.email.message}
                </p>
              )}
            </div>

            {error && (
              <div
                className="flex items-center gap-2 px-3 py-2.5 rounded-xl bg-red-500/10 border border-red-500/20"
                role="alert"
                aria-live="assertive"
              >
                <svg
                  className="w-4 h-4 text-[var(--color-error)] shrink-0"
                  fill="none"
                  viewBox="0 0 24 24"
                  stroke="currentColor"
                  strokeWidth={2}
                >
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    d="M12 9v3.75m9-.75a9 9 0 1 1-18 0 9 9 0 0 1 18 0Zm-9 3.75h.008v.008H12v-.008Z"
                  />
                </svg>
                <p className="text-xs text-[var(--color-error)]">{error}</p>
              </div>
            )}

            <button
              type="submit"
              disabled={loading}
              className="w-full py-3 px-4 bg-gradient-to-r from-[var(--color-primary)] to-[var(--color-primary-hover)] text-white rounded-xl
                hover:opacity-90 transition-all duration-200
                disabled:opacity-50 disabled:cursor-not-allowed font-semibold text-sm
                shadow-lg shadow-[var(--color-primary-glow)]
                active:scale-[0.98]"
            >
              {loading ? (
                <span className="flex items-center justify-center gap-2">
                  <svg className="animate-spin h-4 w-4" fill="none" viewBox="0 0 24 24">
                    <circle
                      className="opacity-25"
                      cx="12"
                      cy="12"
                      r="10"
                      stroke="currentColor"
                      strokeWidth="4"
                    />
                    <path
                      className="opacity-75"
                      fill="currentColor"
                      d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"
                    />
                  </svg>
                  Sending...
                </span>
              ) : (
                'Send Magic Link'
              )}
            </button>
          </form>
        </div>

        <p className="mt-6 text-center text-xs text-[var(--color-text-secondary)]">
          No password needed. We'll email you a secure sign-in link.
        </p>
      </div>
    </div>
  );
}
