import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useForm } from 'react-hook-form';
import { zodResolver } from '@hookform/resolvers/zod';
import { z } from 'zod';
import { useApi } from '@/hooks/useApi';
import { createPortfolioApi } from '@/api/portfolios';
import { createAccountApi } from '@/api/accounts';
import type { AccountType } from '@/types/models';
import { ACCOUNT_TYPE_LABELS } from '@/types/models';

// ── Step schemas ─────────────────────────────────────────────────────

const profileSchema = z.object({
  display_name: z.string().min(1, 'Display name is required'),
  currency: z.string().min(1, 'Currency is required'),
  date_format: z.string().min(1, 'Date format is required'),
});

const portfolioSchema = z.object({
  name: z.string().min(1, 'Portfolio name is required'),
  description: z.string().optional(),
});

const accountSchema = z.object({
  account_type: z.string().min(1, 'Account type is required'),
  name: z.string().min(1, 'Account name is required'),
  institution: z.string().optional(),
  opening_balance: z.number(),
});

type ProfileForm = z.infer<typeof profileSchema>;
type PortfolioForm = z.infer<typeof portfolioSchema>;
type AccountForm = z.infer<typeof accountSchema>;

const ACCOUNT_TYPES: AccountType[] = [
  'checking',
  'savings',
  'credit_card',
  'loan',
  'investment',
  'retirement',
  'cash',
  'other',
];

const STEPS = ['Profile', 'Portfolio', 'Account'] as const;

export function OnboardingPage() {
  const navigate = useNavigate();
  const api = useApi();
  const portfolioApi = createPortfolioApi(api);
  const accountApi = createAccountApi(api);

  const [step, setStep] = useState(0);
  const [profileData, setProfileData] = useState<ProfileForm | null>(null);
  const [portfolioData, setPortfolioData] = useState<PortfolioForm | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [completing, setCompleting] = useState(false);

  const profileForm = useForm<ProfileForm>({
    resolver: zodResolver(profileSchema),
    defaultValues: {
      display_name: '',
      currency: 'USD',
      date_format: 'MM/DD/YYYY',
    },
  });

  const portfolioForm = useForm<PortfolioForm>({
    resolver: zodResolver(portfolioSchema),
    defaultValues: {
      name: 'My Finances',
      description: '',
    },
  });

  const accountForm = useForm<AccountForm>({
    resolver: zodResolver(accountSchema),
    defaultValues: {
      account_type: 'checking',
      name: '',
      institution: '',
      opening_balance: 0,
    },
  });

  const handleProfileNext = (data: ProfileForm) => {
    setProfileData(data);
    setStep(1);
  };

  const handlePortfolioNext = (data: PortfolioForm) => {
    setPortfolioData(data);
    setStep(2);
  };

  const handleComplete = async (data: AccountForm) => {
    if (!profileData || !portfolioData) return;
    setCompleting(true);
    setError(null);

    try {
      // Update user profile (would be a PUT /api/users/me in real impl)
      await api.put('/api/users/me', {
        display_name: profileData.display_name,
        default_currency: profileData.currency,
        date_format: profileData.date_format,
      });

      // Create portfolio
      const portfolio = await portfolioApi.createPortfolio({
        name: portfolioData.name,
        description: portfolioData.description,
      });

      // Create account
      const account = await accountApi.createAccount({
        portfolio_id: portfolio.id,
        name: data.name,
        account_type: data.account_type as AccountType,
        institution: data.institution,
        opening_balance: data.opening_balance,
      });

      navigate(`/accounts/${account.id}`);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Setup failed');
      setCompleting(false);
    }
  };

  const handleSkipAccount = async () => {
    if (!profileData || !portfolioData) return;
    setCompleting(true);
    setError(null);

    try {
      await api.put('/api/users/me', {
        display_name: profileData.display_name,
        default_currency: profileData.currency,
        date_format: profileData.date_format,
      });

      await portfolioApi.createPortfolio({
        name: portfolioData.name,
        description: portfolioData.description,
      });

      navigate('/accounts');
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Setup failed');
      setCompleting(false);
    }
  };

  return (
    <div className="min-h-screen flex items-center justify-center bg-slate-50">
      <div className="w-full max-w-lg">
        {/* Header */}
        <div className="text-center mb-8">
          <h1 className="text-2xl font-bold text-slate-800">Welcome to Finima</h1>
          <p className="text-sm text-slate-500 mt-1">
            Step {step + 1} of {STEPS.length}
          </p>
        </div>

        {/* Progress indicator */}
        <nav aria-label="Setup progress" className="flex items-center justify-center gap-2 mb-8">
          <ol className="flex items-center gap-2">
            {STEPS.map((label, i) => (
              <li
                key={label}
                className="flex items-center"
                aria-current={i === step ? 'step' : undefined}
              >
                {i > 0 && (
                  <div
                    className={`w-12 h-0.5 ${i <= step ? 'bg-blue-600' : 'bg-slate-300'}`}
                    aria-hidden="true"
                  />
                )}
                <div className="flex flex-col items-center">
                  <div
                    className={`w-8 h-8 rounded-full flex items-center justify-center text-sm font-medium ${
                      i < step
                        ? 'bg-blue-600 text-white'
                        : i === step
                          ? 'bg-blue-600 text-white'
                          : 'bg-slate-200 text-slate-500'
                    }`}
                    aria-hidden="true"
                  >
                    {i < step ? '\u2713' : i + 1}
                  </div>
                  <span className="text-xs text-slate-500 mt-1">
                    {label}
                    <span className="sr-only">
                      {i < step ? ' (completed)' : i === step ? ' (current)' : ' (upcoming)'}
                    </span>
                  </span>
                </div>
              </li>
            ))}
          </ol>
        </nav>

        {/* Step content */}
        <div className="bg-white rounded-lg shadow-sm border border-slate-200 p-6">
          {error && (
            <div
              className="mb-4 p-3 bg-red-50 border border-red-200 rounded-lg text-sm text-red-700"
              role="alert"
              aria-live="assertive"
            >
              {error}
            </div>
          )}

          {/* Step 1: Profile */}
          {step === 0 && (
            <form onSubmit={profileForm.handleSubmit(handleProfileNext)} className="space-y-4">
              <h2 className="text-lg font-semibold text-slate-800">Set Up Your Profile</h2>
              <p className="text-sm text-slate-500">
                Tell us a bit about yourself to personalize your experience.
              </p>
              <div>
                <label
                  htmlFor="onb-display-name"
                  className="block text-sm font-medium text-slate-700 mb-1"
                >
                  Display Name
                </label>
                <input
                  id="onb-display-name"
                  aria-required="true"
                  aria-invalid={!!profileForm.formState.errors.display_name}
                  aria-describedby={
                    profileForm.formState.errors.display_name ? 'onb-name-error' : undefined
                  }
                  {...profileForm.register('display_name')}
                  className="w-full px-3 py-2 border border-slate-300 rounded-lg text-sm focus:border-blue-500 focus:ring-1 focus:ring-blue-500"
                  placeholder="Your name"
                />
                {profileForm.formState.errors.display_name && (
                  <p id="onb-name-error" className="text-xs text-red-500 mt-1" role="alert">
                    {profileForm.formState.errors.display_name.message}
                  </p>
                )}
              </div>
              <div>
                <label
                  htmlFor="onb-currency"
                  className="block text-sm font-medium text-slate-700 mb-1"
                >
                  Currency
                </label>
                <select
                  id="onb-currency"
                  {...profileForm.register('currency')}
                  className="w-full px-3 py-2 border border-slate-300 rounded-lg text-sm focus:border-blue-500 focus:ring-1 focus:ring-blue-500"
                >
                  <option value="USD">USD ($)</option>
                  <option value="EUR">EUR</option>
                  <option value="GBP">GBP</option>
                  <option value="CAD">CAD</option>
                  <option value="AUD">AUD</option>
                  <option value="JPY">JPY</option>
                </select>
              </div>
              <div>
                <label
                  htmlFor="onb-date-format"
                  className="block text-sm font-medium text-slate-700 mb-1"
                >
                  Date Format
                </label>
                <select
                  id="onb-date-format"
                  {...profileForm.register('date_format')}
                  className="w-full px-3 py-2 border border-slate-300 rounded-lg text-sm focus:border-blue-500 focus:ring-1 focus:ring-blue-500"
                >
                  <option value="MM/DD/YYYY">MM/DD/YYYY</option>
                  <option value="DD/MM/YYYY">DD/MM/YYYY</option>
                  <option value="YYYY-MM-DD">YYYY-MM-DD</option>
                </select>
              </div>
              <div className="flex justify-end pt-2">
                <button
                  type="submit"
                  className="px-6 py-2 bg-blue-600 text-white text-sm font-medium rounded-lg hover:bg-blue-700 transition-colors"
                >
                  Next
                </button>
              </div>
            </form>
          )}

          {/* Step 2: Portfolio */}
          {step === 1 && (
            <form onSubmit={portfolioForm.handleSubmit(handlePortfolioNext)} className="space-y-4">
              <h2 className="text-lg font-semibold text-slate-800">Create Your Portfolio</h2>
              <p className="text-sm text-slate-500">
                A portfolio groups all your accounts together.
              </p>
              <div>
                <label
                  htmlFor="onb-portfolio-name"
                  className="block text-sm font-medium text-slate-700 mb-1"
                >
                  Name *
                </label>
                <input
                  id="onb-portfolio-name"
                  aria-required="true"
                  aria-invalid={!!portfolioForm.formState.errors.name}
                  aria-describedby={
                    portfolioForm.formState.errors.name ? 'onb-portfolio-name-error' : undefined
                  }
                  {...portfolioForm.register('name')}
                  className="w-full px-3 py-2 border border-slate-300 rounded-lg text-sm focus:border-blue-500 focus:ring-1 focus:ring-blue-500"
                  placeholder="My Finances"
                />
                {portfolioForm.formState.errors.name && (
                  <p
                    id="onb-portfolio-name-error"
                    className="text-xs text-red-500 mt-1"
                    role="alert"
                  >
                    {portfolioForm.formState.errors.name.message}
                  </p>
                )}
              </div>
              <div>
                <label
                  htmlFor="onb-portfolio-desc"
                  className="block text-sm font-medium text-slate-700 mb-1"
                >
                  Description (optional)
                </label>
                <textarea
                  id="onb-portfolio-desc"
                  {...portfolioForm.register('description')}
                  rows={2}
                  className="w-full px-3 py-2 border border-slate-300 rounded-lg text-sm focus:border-blue-500 focus:ring-1 focus:ring-blue-500"
                />
              </div>
              <div className="flex justify-between pt-2">
                <button
                  type="button"
                  onClick={() => setStep(0)}
                  className="px-6 py-2 bg-slate-100 text-slate-700 text-sm font-medium rounded-lg hover:bg-slate-200 transition-colors"
                >
                  Back
                </button>
                <button
                  type="submit"
                  className="px-6 py-2 bg-blue-600 text-white text-sm font-medium rounded-lg hover:bg-blue-700 transition-colors"
                >
                  Next
                </button>
              </div>
            </form>
          )}

          {/* Step 3: Account */}
          {step === 2 && (
            <form onSubmit={accountForm.handleSubmit(handleComplete)} className="space-y-4">
              <h2 className="text-lg font-semibold text-slate-800">Add Your First Account</h2>
              <p className="text-sm text-slate-500">
                Set up your first bank account to start tracking.
              </p>
              <div>
                <label
                  htmlFor="onb-acct-type"
                  className="block text-sm font-medium text-slate-700 mb-1"
                >
                  Account Type
                </label>
                <select
                  id="onb-acct-type"
                  {...accountForm.register('account_type')}
                  className="w-full px-3 py-2 border border-slate-300 rounded-lg text-sm focus:border-blue-500 focus:ring-1 focus:ring-blue-500"
                >
                  {ACCOUNT_TYPES.map((t) => (
                    <option key={t} value={t}>
                      {ACCOUNT_TYPE_LABELS[t]}
                    </option>
                  ))}
                </select>
              </div>
              <div>
                <label
                  htmlFor="onb-acct-name"
                  className="block text-sm font-medium text-slate-700 mb-1"
                >
                  Name *
                </label>
                <input
                  id="onb-acct-name"
                  aria-required="true"
                  aria-invalid={!!accountForm.formState.errors.name}
                  aria-describedby={
                    accountForm.formState.errors.name ? 'onb-acct-name-error' : undefined
                  }
                  {...accountForm.register('name')}
                  className="w-full px-3 py-2 border border-slate-300 rounded-lg text-sm focus:border-blue-500 focus:ring-1 focus:ring-blue-500"
                  placeholder="e.g. Chase Checking"
                />
                {accountForm.formState.errors.name && (
                  <p id="onb-acct-name-error" className="text-xs text-red-500 mt-1" role="alert">
                    {accountForm.formState.errors.name.message}
                  </p>
                )}
              </div>
              <div>
                <label
                  htmlFor="onb-acct-institution"
                  className="block text-sm font-medium text-slate-700 mb-1"
                >
                  Institution
                </label>
                <input
                  id="onb-acct-institution"
                  {...accountForm.register('institution')}
                  className="w-full px-3 py-2 border border-slate-300 rounded-lg text-sm focus:border-blue-500 focus:ring-1 focus:ring-blue-500"
                  placeholder="e.g. Chase Bank"
                />
              </div>
              <div>
                <label
                  htmlFor="onb-acct-balance"
                  className="block text-sm font-medium text-slate-700 mb-1"
                >
                  Opening Balance
                </label>
                <input
                  id="onb-acct-balance"
                  {...accountForm.register('opening_balance', { valueAsNumber: true })}
                  type="number"
                  step="0.01"
                  className="w-full px-3 py-2 border border-slate-300 rounded-lg text-sm focus:border-blue-500 focus:ring-1 focus:ring-blue-500"
                />
              </div>
              <div className="flex justify-between pt-2">
                <div className="flex gap-3">
                  <button
                    type="button"
                    onClick={() => setStep(1)}
                    className="px-6 py-2 bg-slate-100 text-slate-700 text-sm font-medium rounded-lg hover:bg-slate-200 transition-colors"
                  >
                    Back
                  </button>
                  <button
                    type="button"
                    onClick={handleSkipAccount}
                    disabled={completing}
                    className="px-6 py-2 text-slate-500 text-sm font-medium hover:text-slate-700 transition-colors"
                  >
                    Skip
                  </button>
                </div>
                <button
                  type="submit"
                  disabled={completing}
                  className="px-6 py-2 bg-blue-600 text-white text-sm font-medium rounded-lg hover:bg-blue-700 disabled:opacity-50 transition-colors"
                >
                  {completing ? 'Setting up...' : 'Complete Setup'}
                </button>
              </div>
            </form>
          )}
        </div>
      </div>
    </div>
  );
}
