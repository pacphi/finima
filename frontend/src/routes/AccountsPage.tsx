import { useState, useEffect, useMemo } from 'react';
import { useNavigate } from 'react-router-dom';
import { useForm } from 'react-hook-form';
import { zodResolver } from '@hookform/resolvers/zod';
import { z } from 'zod';
import { useApi } from '@/hooks/useApi';
import { createPortfolioApi } from '@/api/portfolios';
import { createAccountApi } from '@/api/accounts';
import { formatCurrency } from '@/utils/format';
import { usePortfolioStore } from '@/stores/portfolioStore';
import { useFocusTrap } from '@/hooks/useFocusTrap';
import type { Account, AccountType, Portfolio } from '@/types/models';
import { ACCOUNT_TYPE_LABELS, ACCOUNT_TYPE_ICONS } from '@/types/models';

function formatDate(dateStr: string | null): string {
  if (!dateStr) return 'Never';
  return new Date(dateStr).toLocaleDateString('en-US', {
    month: 'short',
    day: 'numeric',
    year: 'numeric',
  });
}

const addAccountSchema = z.object({
  portfolio_id: z.string().min(1, 'Portfolio is required'),
  name: z.string().min(1, 'Name is required'),
  account_type: z.string().min(1, 'Account type is required'),
  institution: z.string().optional(),
  currency: z.string(),
  opening_balance: z.number(),
  notes: z.string().optional(),
});

type AddAccountForm = z.infer<typeof addAccountSchema>;

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

const LIABILITY_TYPES = new Set<AccountType>(['credit_card', 'loan']);

export function AccountsPage() {
  const navigate = useNavigate();
  const api = useApi();
  const portfolioApi = createPortfolioApi(api);
  const accountApi = createAccountApi(api);

  const portfolios = usePortfolioStore((s) => s.portfolios);
  const activePortfolioId = usePortfolioStore((s) => s.activePortfolioId);
  const accounts = usePortfolioStore((s) => s.accounts);
  const setPortfolios = usePortfolioStore((s) => s.setPortfolios);
  const selectPortfolio = usePortfolioStore((s) => s.selectPortfolio);
  const setAccounts = usePortfolioStore((s) => s.setAccounts);
  const addAccount = usePortfolioStore((s) => s.addAccount);
  const addPortfolio = usePortfolioStore((s) => s.addPortfolio);

  const [showAddModal, setShowAddModal] = useState(false);
  const [loading, setLoading] = useState(true);
  const [archiving, setArchiving] = useState<string | null>(null);
  const [showPortfolioModal, setShowPortfolioModal] = useState(false);
  const [editingPortfolio, setEditingPortfolio] = useState<Portfolio | null>(null);
  const [portfolioName, setPortfolioName] = useState('');
  const [portfolioDesc, setPortfolioDesc] = useState('');
  const [savingPortfolio, setSavingPortfolio] = useState(false);

  const {
    register,
    handleSubmit,
    reset,
    formState: { errors, isSubmitting },
  } = useForm<AddAccountForm>({
    resolver: zodResolver(addAccountSchema),
    defaultValues: {
      portfolio_id: activePortfolioId ?? '',
      currency: 'USD',
      opening_balance: 0,
    },
  });

  useEffect(() => {
    portfolioApi
      .listPortfolios()
      .then((p) => {
        setPortfolios(p);
      })
      .catch(console.error);
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    if (!activePortfolioId) {
      setLoading(false);
      return;
    }
    setLoading(true);
    let cancelled = false;
    accountApi
      .listAccounts(activePortfolioId)
      .then((a) => {
        if (!cancelled) setAccounts(a);
      })
      .catch(console.error)
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [activePortfolioId]); // eslint-disable-line react-hooks/exhaustive-deps

  const { totalAssets, totalLiabilities, netWorth } = useMemo(() => {
    let assets = 0;
    let liabilities = 0;
    for (const a of accounts) {
      if (LIABILITY_TYPES.has(a.account_type)) {
        liabilities += Math.abs(a.current_balance);
      } else {
        assets += a.current_balance;
      }
    }
    return {
      totalAssets: assets,
      totalLiabilities: liabilities,
      netWorth: assets - liabilities,
    };
  }, [accounts]);

  const onSubmit = async (data: AddAccountForm) => {
    try {
      const created = await accountApi.createAccount({
        portfolio_id: data.portfolio_id,
        name: data.name,
        account_type: data.account_type as AccountType,
        institution: data.institution,
        currency: data.currency,
        opening_balance: data.opening_balance,
        notes: data.notes,
      });
      addAccount(created);
      setShowAddModal(false);
      reset();
    } catch (err) {
      console.error('Failed to create account:', err);
    }
  };

  const handleOpenPortfolioModal = (portfolio?: Portfolio) => {
    if (portfolio) {
      setEditingPortfolio(portfolio);
      setPortfolioName(portfolio.name);
      setPortfolioDesc(portfolio.description ?? '');
    } else {
      setEditingPortfolio(null);
      setPortfolioName('');
      setPortfolioDesc('');
    }
    setShowPortfolioModal(true);
  };

  const handleSavePortfolio = async () => {
    if (!portfolioName.trim()) return;
    setSavingPortfolio(true);
    try {
      if (editingPortfolio) {
        const updated = await portfolioApi.updatePortfolio(editingPortfolio.id, {
          name: portfolioName.trim(),
          description: portfolioDesc.trim() || undefined,
        });
        setPortfolios(portfolios.map((p) => (p.id === updated.id ? updated : p)));
      } else {
        const created = await portfolioApi.createPortfolio({
          name: portfolioName.trim(),
          description: portfolioDesc.trim() || undefined,
        });
        addPortfolio(created);
      }
      setShowPortfolioModal(false);
    } catch (err) {
      console.error('Failed to save portfolio:', err);
    } finally {
      setSavingPortfolio(false);
    }
  };

  const handleArchive = async (e: React.MouseEvent, accountId: string) => {
    e.stopPropagation();
    if (!confirm('Archive this account? It will be hidden from your account list.')) return;
    setArchiving(accountId);
    try {
      await accountApi.archiveAccount(accountId);
      setAccounts(accounts.filter((a) => a.id !== accountId));
    } catch (err) {
      console.error('Failed to archive account:', err);
    } finally {
      setArchiving(null);
    }
  };

  const modalRef = useFocusTrap<HTMLDivElement>(showAddModal, () => {
    setShowAddModal(false);
    reset();
  });

  return (
    <div className="p-6">
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-2xl font-bold text-slate-800">Accounts</h1>
        {portfolios.length > 0 && (
          <button
            onClick={() => setShowAddModal(true)}
            className="px-4 py-2 bg-blue-600 text-white text-sm font-medium rounded-lg hover:bg-blue-700 transition-colors"
          >
            + Add Account
          </button>
        )}
      </div>

      {/* Portfolio selector */}
      <div className="mb-6 flex items-end gap-3">
        {portfolios.length > 1 && (
          <div>
            <label className="block text-sm font-medium text-slate-600 mb-1">Portfolio</label>
            <select
              value={activePortfolioId ?? ''}
              onChange={(e) => selectPortfolio(e.target.value)}
              className="px-3 py-2 border border-slate-300 rounded-lg text-sm focus:border-blue-500 focus:ring-1 focus:ring-blue-500"
            >
              {portfolios.map((p: Portfolio) => (
                <option key={p.id} value={p.id}>
                  {p.name}
                </option>
              ))}
            </select>
          </div>
        )}
        {activePortfolioId && (
          <button
            onClick={() => {
              const p = portfolios.find((p) => p.id === activePortfolioId);
              if (p) handleOpenPortfolioModal(p);
            }}
            className="px-3 py-2 text-sm text-slate-600 border border-slate-300 rounded-lg hover:bg-slate-50 transition-colors"
          >
            Edit Portfolio
          </button>
        )}
        <button
          onClick={() => handleOpenPortfolioModal()}
          className="px-3 py-2 text-sm font-medium text-white bg-blue-600 rounded-lg hover:bg-blue-700 transition-colors"
        >
          + New Portfolio
        </button>
      </div>

      {loading ? (
        <div className="text-center py-12 text-slate-400">Loading accounts...</div>
      ) : portfolios.length === 0 ? (
        <div className="text-center py-12">
          <p className="text-slate-400 mb-2">Create a portfolio to get started</p>
          <p className="text-sm text-slate-400">
            You need at least one portfolio before adding accounts.
          </p>
        </div>
      ) : accounts.length === 0 ? (
        <div className="text-center py-12">
          <p className="text-slate-400">No accounts yet</p>
        </div>
      ) : (
        <>
          {/* Account cards */}
          <div className="space-y-3 mb-6">
            {accounts.map((account: Account) => (
              <button
                key={account.id}
                onClick={() => navigate(`/accounts/${account.id}`)}
                className="w-full text-left bg-white rounded-lg shadow-sm border border-slate-200 p-4 hover:border-blue-300 hover:shadow transition-all"
              >
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-3">
                    <span className="text-xl">{ACCOUNT_TYPE_ICONS[account.account_type]}</span>
                    <div>
                      <div className="flex items-center gap-2">
                        <span className="font-medium text-slate-800">{account.name}</span>
                        <span className="inline-block px-2 py-0.5 text-xs font-medium bg-slate-100 text-slate-600 rounded-full">
                          {ACCOUNT_TYPE_LABELS[account.account_type]}
                        </span>
                      </div>
                      <p className="text-sm text-slate-500">
                        {account.institution ?? 'No institution'}
                        {' · '}
                        Last import: {formatDate(account.last_import_at)}
                        {' · '}
                        {account.transaction_count} transactions
                      </p>
                    </div>
                  </div>
                  <div className="flex items-center gap-3">
                    <span
                      className={`text-lg font-bold ${
                        account.current_balance >= 0 ? 'text-slate-800' : 'text-red-600'
                      }`}
                    >
                      {formatCurrency(account.current_balance)}
                      <span className="sr-only">
                        {account.current_balance >= 0
                          ? ' (positive balance)'
                          : ' (negative balance)'}
                      </span>
                    </span>
                    <button
                      onClick={(e) => void handleArchive(e, account.id)}
                      disabled={archiving === account.id}
                      className="px-2 py-1 text-xs text-slate-500 hover:text-red-600 hover:bg-red-50 rounded transition-colors disabled:opacity-50"
                      aria-label={`Archive account: ${account.name}`}
                    >
                      {archiving === account.id ? 'Archiving...' : 'Archive'}
                    </button>
                  </div>
                </div>
              </button>
            ))}
          </div>

          {/* Summary row */}
          <div className="bg-slate-50 rounded-lg border border-slate-200 p-4 flex items-center justify-between text-sm">
            <div>
              <span className="text-slate-500">Total Assets: </span>
              <span className="font-medium text-slate-800">{formatCurrency(totalAssets)}</span>
            </div>
            <div>
              <span className="text-slate-500">Total Liabilities: </span>
              <span className="font-medium text-red-600">-{formatCurrency(totalLiabilities)}</span>
            </div>
            <div>
              <span className="text-slate-500">Net Worth: </span>
              <span className={`font-bold ${netWorth >= 0 ? 'text-green-700' : 'text-red-600'}`}>
                {formatCurrency(netWorth)}
              </span>
            </div>
          </div>
        </>
      )}

      {/* Portfolio Modal */}
      {showPortfolioModal && (
        <div className="fixed inset-0 z-50 flex items-center justify-center">
          <div
            className="absolute inset-0 bg-black/50"
            onClick={() => setShowPortfolioModal(false)}
            aria-hidden="true"
          />
          <div
            role="dialog"
            aria-modal="true"
            aria-labelledby="portfolio-modal-title"
            className="relative bg-white rounded-lg shadow-xl w-full max-w-md p-6 z-10"
          >
            <h2 id="portfolio-modal-title" className="text-lg font-semibold text-slate-800 mb-4">
              {editingPortfolio ? 'Edit Portfolio' : 'New Portfolio'}
            </h2>
            <div className="space-y-4">
              <div>
                <label className="block text-sm font-medium text-slate-700 mb-1">Name</label>
                <input
                  type="text"
                  value={portfolioName}
                  onChange={(e) => setPortfolioName(e.target.value)}
                  className="w-full px-3 py-2 border border-slate-300 rounded-lg text-sm focus:border-blue-500 focus:ring-1 focus:ring-blue-500"
                  placeholder="My Portfolio"
                />
              </div>
              <div>
                <label className="block text-sm font-medium text-slate-700 mb-1">
                  Description (optional)
                </label>
                <textarea
                  value={portfolioDesc}
                  onChange={(e) => setPortfolioDesc(e.target.value)}
                  rows={2}
                  className="w-full px-3 py-2 border border-slate-300 rounded-lg text-sm focus:border-blue-500 focus:ring-1 focus:ring-blue-500"
                />
              </div>
            </div>
            <div className="flex justify-end gap-3 pt-4">
              <button
                type="button"
                onClick={() => setShowPortfolioModal(false)}
                className="px-4 py-2 bg-slate-100 text-slate-700 text-sm font-medium rounded-lg hover:bg-slate-200 transition-colors"
              >
                Cancel
              </button>
              <button
                onClick={() => void handleSavePortfolio()}
                disabled={savingPortfolio || !portfolioName.trim()}
                className="px-4 py-2 bg-blue-600 text-white text-sm font-medium rounded-lg hover:bg-blue-700 disabled:opacity-50 transition-colors"
              >
                {savingPortfolio ? 'Saving...' : editingPortfolio ? 'Update' : 'Create'}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Add Account Modal */}
      {showAddModal && (
        <div className="fixed inset-0 z-50 flex items-center justify-center">
          <div
            className="absolute inset-0 bg-black/50"
            onClick={() => setShowAddModal(false)}
            aria-hidden="true"
          />
          <div
            ref={modalRef}
            role="dialog"
            aria-modal="true"
            aria-labelledby="add-account-title"
            className="relative bg-white rounded-lg shadow-xl w-full max-w-md p-6 z-10"
            tabIndex={-1}
          >
            <h2 id="add-account-title" className="text-lg font-semibold text-slate-800 mb-4">
              Add Account
            </h2>
            <form onSubmit={handleSubmit(onSubmit)} className="space-y-4">
              <div>
                <label className="block text-sm font-medium text-slate-700 mb-1">Portfolio</label>
                <select
                  {...register('portfolio_id')}
                  className="w-full px-3 py-2 border border-slate-300 rounded-lg text-sm focus:border-blue-500 focus:ring-1 focus:ring-blue-500"
                >
                  {portfolios.map((p: Portfolio) => (
                    <option key={p.id} value={p.id}>
                      {p.name}
                    </option>
                  ))}
                </select>
                {errors.portfolio_id && (
                  <p className="text-xs text-red-500 mt-1">{errors.portfolio_id.message}</p>
                )}
              </div>
              <div>
                <label
                  htmlFor="add-acct-type"
                  className="block text-sm font-medium text-slate-700 mb-1"
                >
                  Account Type
                </label>
                <select
                  id="add-acct-type"
                  aria-required="true"
                  aria-invalid={!!errors.account_type}
                  aria-describedby={errors.account_type ? 'acct-type-error' : undefined}
                  {...register('account_type')}
                  className="w-full px-3 py-2 border border-slate-300 rounded-lg text-sm focus:border-blue-500 focus:ring-1 focus:ring-blue-500"
                >
                  <option value="">Select type...</option>
                  {ACCOUNT_TYPES.map((t) => (
                    <option key={t} value={t}>
                      {ACCOUNT_TYPE_LABELS[t]}
                    </option>
                  ))}
                </select>
                {errors.account_type && (
                  <p id="acct-type-error" className="text-xs text-red-500 mt-1" role="alert">
                    {errors.account_type.message}
                  </p>
                )}
              </div>
              <div>
                <label
                  htmlFor="add-acct-name"
                  className="block text-sm font-medium text-slate-700 mb-1"
                >
                  Name
                </label>
                <input
                  id="add-acct-name"
                  aria-required="true"
                  aria-invalid={!!errors.name}
                  aria-describedby={errors.name ? 'acct-name-error' : undefined}
                  {...register('name')}
                  className="w-full px-3 py-2 border border-slate-300 rounded-lg text-sm focus:border-blue-500 focus:ring-1 focus:ring-blue-500"
                  placeholder="e.g. Chase Checking"
                />
                {errors.name && (
                  <p id="acct-name-error" className="text-xs text-red-500 mt-1" role="alert">
                    {errors.name.message}
                  </p>
                )}
              </div>
              <div>
                <label className="block text-sm font-medium text-slate-700 mb-1">Institution</label>
                <input
                  {...register('institution')}
                  className="w-full px-3 py-2 border border-slate-300 rounded-lg text-sm focus:border-blue-500 focus:ring-1 focus:ring-blue-500"
                  placeholder="e.g. Chase Bank"
                />
              </div>
              <div className="grid grid-cols-2 gap-4">
                <div>
                  <label className="block text-sm font-medium text-slate-700 mb-1">Currency</label>
                  <select
                    {...register('currency')}
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
                  <label className="block text-sm font-medium text-slate-700 mb-1">
                    Opening Balance
                  </label>
                  <input
                    {...register('opening_balance', { valueAsNumber: true })}
                    type="number"
                    step="0.01"
                    className="w-full px-3 py-2 border border-slate-300 rounded-lg text-sm focus:border-blue-500 focus:ring-1 focus:ring-blue-500"
                  />
                </div>
              </div>
              <div>
                <label className="block text-sm font-medium text-slate-700 mb-1">Notes</label>
                <textarea
                  {...register('notes')}
                  rows={2}
                  className="w-full px-3 py-2 border border-slate-300 rounded-lg text-sm focus:border-blue-500 focus:ring-1 focus:ring-blue-500"
                />
              </div>
              <div className="flex justify-end gap-3 pt-2">
                <button
                  type="button"
                  onClick={() => {
                    setShowAddModal(false);
                    reset();
                  }}
                  className="px-4 py-2 bg-slate-100 text-slate-700 text-sm font-medium rounded-lg hover:bg-slate-200 transition-colors"
                >
                  Cancel
                </button>
                <button
                  type="submit"
                  disabled={isSubmitting}
                  className="px-4 py-2 bg-blue-600 text-white text-sm font-medium rounded-lg hover:bg-blue-700 disabled:opacity-50 transition-colors"
                >
                  {isSubmitting ? 'Creating...' : 'Create Account'}
                </button>
              </div>
            </form>
          </div>
        </div>
      )}
    </div>
  );
}
