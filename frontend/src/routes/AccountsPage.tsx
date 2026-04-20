import { useState, useEffect, useMemo } from 'react';
import { useNavigate } from 'react-router-dom';
import { useForm } from 'react-hook-form';
import { zodResolver } from '@hookform/resolvers/zod';
import { z } from 'zod';
import { useApi } from '@/hooks/useApi';
import { createAccountApi } from '@/api/accounts';
import { formatCurrency } from '@/utils/format';
import { usePortfolioStore } from '@/stores/portfolioStore';
import { useFocusTrap } from '@/hooks/useFocusTrap';
import type { Account, AccountType, Portfolio } from '@/types/models';
import { ACCOUNT_TYPE_LABELS, ACCOUNT_TYPE_ICONS, classifyBalance } from '@/types/models';

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

export function AccountsPage() {
  const navigate = useNavigate();
  const api = useApi();
  const accountApi = createAccountApi(api);

  const portfolios = usePortfolioStore((s) => s.portfolios);
  const activePortfolioId = usePortfolioStore((s) => s.activePortfolioId);
  const accounts = usePortfolioStore((s) => s.accounts);
  const setAccounts = usePortfolioStore((s) => s.setAccounts);
  const addAccount = usePortfolioStore((s) => s.addAccount);

  const [showAddModal, setShowAddModal] = useState(false);
  const [fetchingAccounts, setFetchingAccounts] = useState(true);
  const loading = !!activePortfolioId && fetchingAccounts;
  const [archiving, setArchiving] = useState<string | null>(null);
  const [settingPrimary, setSettingPrimary] = useState<string | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<Account | null>(null);
  const [deleteConfirmText, setDeleteConfirmText] = useState('');
  const [deleting, setDeleting] = useState(false);
  const [deleteError, setDeleteError] = useState<string | null>(null);

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
    if (!activePortfolioId) return;
    let cancelled = false;
    accountApi
      .listAccounts(activePortfolioId)
      .then((a) => {
        if (!cancelled) setAccounts(a);
      })
      .catch(console.error)
      .finally(() => {
        if (!cancelled) setFetchingAccounts(false);
      });
    return () => {
      cancelled = true;
    };
  }, [activePortfolioId]); // eslint-disable-line react-hooks/exhaustive-deps

  // Canonical-amount split lives in `classifyBalance` (see ADR-018
  // and its backend twin `AccountRole::classify_balance`). Net worth
  // is simply the sum of every signed balance — no abs().
  const { totalAssets, totalLiabilities, netWorth } = useMemo(() => {
    let assets = 0;
    let liabilities = 0;
    let net = 0;
    for (const a of accounts) {
      const { asset, liability } = classifyBalance(a.account_type, a.current_balance);
      assets += asset;
      liabilities += liability;
      net += a.current_balance;
    }
    return {
      totalAssets: assets,
      totalLiabilities: liabilities,
      netWorth: net,
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

  const handleSetPrimary = async (e: React.MouseEvent, accountId: string) => {
    e.stopPropagation();
    setSettingPrimary(accountId);
    try {
      await accountApi.setPrimary(accountId);
      // Refresh accounts to reflect the change.
      if (activePortfolioId) {
        const refreshed = await accountApi.listAccounts(activePortfolioId);
        setAccounts(refreshed);
      }
    } catch (err) {
      console.error('Failed to set primary account:', err);
    } finally {
      setSettingPrimary(null);
    }
  };

  const openDeleteModal = (e: React.MouseEvent, account: Account) => {
    e.stopPropagation();
    setDeleteTarget(account);
    setDeleteConfirmText('');
    setDeleteError(null);
  };

  const handleDeleteAccount = async () => {
    if (!deleteTarget) return;
    setDeleting(true);
    setDeleteError(null);
    try {
      await accountApi.deleteAccount(deleteTarget.id);
      setAccounts(accounts.filter((a) => a.id !== deleteTarget.id));
      setDeleteTarget(null);
      setDeleteConfirmText('');
    } catch (err) {
      console.error('Failed to delete account:', err);
      setDeleteError(err instanceof Error ? err.message : 'Failed to delete account');
    } finally {
      setDeleting(false);
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
    <div className="p-6 lg:p-8">
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-2xl font-bold text-[var(--color-text)] tracking-tight">Accounts</h1>
        {portfolios.length > 0 && (
          <button onClick={() => setShowAddModal(true)} className="btn-primary">
            + Add Account
          </button>
        )}
      </div>

      {loading ? (
        <div className="text-center py-12 text-[var(--color-text-secondary)]">
          Loading accounts...
        </div>
      ) : portfolios.length === 0 ? (
        <div className="text-center py-12">
          <p className="text-[var(--color-text-secondary)] mb-2">
            Create a portfolio to get started
          </p>
          <p className="text-sm text-[var(--color-text-secondary)]">
            Go to{' '}
            <button
              onClick={() => navigate('/portfolios')}
              className="text-[var(--color-primary)] hover:underline"
            >
              Portfolios
            </button>{' '}
            to create one before adding accounts.
          </p>
        </div>
      ) : accounts.length === 0 ? (
        <div className="text-center py-12">
          <p className="text-[var(--color-text-secondary)]">No accounts yet</p>
        </div>
      ) : (
        <>
          {/* Account cards */}
          <div className="space-y-3 mb-6">
            {accounts.map((account: Account) => (
              <button
                key={account.id}
                onClick={() => navigate(`/accounts/${account.id}`)}
                className="w-full text-left bg-[var(--color-card)] backdrop-blur-sm rounded-2xl border border-[var(--color-border)] p-5 hover:border-[var(--color-primary-muted)] hover:shadow-[var(--card-shadow-hover)] transition-all"
              >
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-3">
                    <span className="text-xl">{ACCOUNT_TYPE_ICONS[account.account_type]}</span>
                    <div>
                      <div className="flex items-center gap-2">
                        <span className="font-medium text-[var(--color-text)]">{account.name}</span>
                        <span className="badge-primary">
                          {ACCOUNT_TYPE_LABELS[account.account_type]}
                        </span>
                        {account.is_primary_income && (
                          <span
                            className="inline-flex items-center gap-1 rounded-full bg-amber-500/10 px-2 py-0.5 text-xs font-medium text-amber-500"
                            title="Primary income account"
                          >
                            Primary
                          </span>
                        )}
                      </div>
                      <p className="text-sm text-[var(--color-text-secondary)]">
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
                        account.current_balance >= 0
                          ? 'text-[var(--color-text)]'
                          : 'text-[var(--color-error)]'
                      }`}
                    >
                      {formatCurrency(account.current_balance)}
                      <span className="sr-only">
                        {account.current_balance >= 0
                          ? ' (positive balance)'
                          : ' (negative balance)'}
                      </span>
                    </span>
                    {!account.is_primary_income && (
                      <button
                        onClick={(e) => void handleSetPrimary(e, account.id)}
                        disabled={settingPrimary === account.id}
                        className="px-2 py-1 text-xs text-[var(--color-text-secondary)] hover:text-amber-500 hover:bg-amber-500/10 rounded-lg transition-colors disabled:opacity-50"
                        aria-label={`Set ${account.name} as primary income account`}
                      >
                        {settingPrimary === account.id ? 'Setting...' : 'Set Primary'}
                      </button>
                    )}
                    <button
                      onClick={(e) => void handleArchive(e, account.id)}
                      disabled={archiving === account.id}
                      className="px-2 py-1 text-xs text-[var(--color-text-secondary)] hover:text-[var(--color-error)] hover:bg-[var(--color-error)]/10 rounded-lg transition-colors disabled:opacity-50"
                      aria-label={`Archive account: ${account.name}`}
                    >
                      {archiving === account.id ? 'Archiving...' : 'Archive'}
                    </button>
                    <button
                      onClick={(e) => openDeleteModal(e, account)}
                      className="px-2 py-1 text-xs text-red-400 hover:text-white hover:bg-red-600 rounded-lg transition-colors"
                      aria-label={`Permanently delete account: ${account.name}`}
                      title="Permanently delete this account and all its data"
                    >
                      Delete
                    </button>
                  </div>
                </div>
              </button>
            ))}
          </div>

          {/* Summary row */}
          <div className="bg-[var(--color-card)] backdrop-blur-sm rounded-2xl border border-[var(--color-border)] p-5 flex items-center justify-between text-sm">
            <div>
              <span className="text-[var(--color-text-secondary)]">Total Assets: </span>
              <span className="font-medium text-[var(--color-text)]">
                {formatCurrency(totalAssets)}
              </span>
            </div>
            <div>
              <span className="text-[var(--color-text-secondary)]">Total Liabilities: </span>
              <span className="font-medium text-[var(--color-error)]">
                -{formatCurrency(totalLiabilities)}
              </span>
            </div>
            <div>
              <span className="text-[var(--color-text-secondary)]">Net Worth: </span>
              <span
                className={`font-bold ${netWorth >= 0 ? 'text-[var(--color-primary)]' : 'text-[var(--color-error)]'}`}
              >
                {formatCurrency(netWorth)}
              </span>
            </div>
          </div>
        </>
      )}

      {/* Add Account Modal */}
      {showAddModal && (
        <div className="fixed inset-0 z-50 flex items-center justify-center">
          <div
            className="absolute inset-0 bg-black/60 backdrop-blur-sm"
            onClick={() => setShowAddModal(false)}
            aria-hidden="true"
          />
          <div
            ref={modalRef}
            role="dialog"
            aria-modal="true"
            aria-labelledby="add-account-title"
            className="relative bg-[var(--color-surface)] border border-[var(--color-border)] rounded-2xl shadow-xl w-full max-w-md p-6 z-10 max-h-[85vh] overflow-y-auto"
            tabIndex={-1}
          >
            <h2
              id="add-account-title"
              className="text-lg font-semibold text-[var(--color-text)] mb-4"
            >
              Add Account
            </h2>
            <form onSubmit={handleSubmit(onSubmit)} className="space-y-4">
              <div>
                <label className="block text-xs font-medium text-[var(--color-text-secondary)] mb-1 uppercase tracking-wider">
                  Portfolio
                </label>
                <select {...register('portfolio_id')} className="input-themed">
                  {portfolios.map((p: Portfolio) => (
                    <option key={p.id} value={p.id}>
                      {p.name}
                    </option>
                  ))}
                </select>
                {errors.portfolio_id && (
                  <p className="text-xs text-[var(--color-error)] mt-1">
                    {errors.portfolio_id.message}
                  </p>
                )}
              </div>
              <div>
                <label
                  htmlFor="add-acct-type"
                  className="block text-xs font-medium text-[var(--color-text-secondary)] mb-1 uppercase tracking-wider"
                >
                  Account Type
                </label>
                <select
                  id="add-acct-type"
                  aria-required="true"
                  aria-invalid={!!errors.account_type}
                  aria-describedby={errors.account_type ? 'acct-type-error' : undefined}
                  {...register('account_type')}
                  className="input-themed"
                >
                  <option value="">Select type...</option>
                  {ACCOUNT_TYPES.map((t) => (
                    <option key={t} value={t}>
                      {ACCOUNT_TYPE_LABELS[t]}
                    </option>
                  ))}
                </select>
                {errors.account_type && (
                  <p
                    id="acct-type-error"
                    className="text-xs text-[var(--color-error)] mt-1"
                    role="alert"
                  >
                    {errors.account_type.message}
                  </p>
                )}
              </div>
              <div>
                <label
                  htmlFor="add-acct-name"
                  className="block text-xs font-medium text-[var(--color-text-secondary)] mb-1 uppercase tracking-wider"
                >
                  Name
                </label>
                <input
                  id="add-acct-name"
                  aria-required="true"
                  aria-invalid={!!errors.name}
                  aria-describedby={errors.name ? 'acct-name-error' : undefined}
                  {...register('name')}
                  className="input-themed"
                  placeholder="e.g. Chase Checking"
                />
                {errors.name && (
                  <p
                    id="acct-name-error"
                    className="text-xs text-[var(--color-error)] mt-1"
                    role="alert"
                  >
                    {errors.name.message}
                  </p>
                )}
              </div>
              <div>
                <label className="block text-xs font-medium text-[var(--color-text-secondary)] mb-1 uppercase tracking-wider">
                  Institution
                </label>
                <input
                  {...register('institution')}
                  className="input-themed"
                  placeholder="e.g. Chase Bank"
                />
              </div>
              <div className="grid grid-cols-2 gap-4">
                <div>
                  <label className="block text-xs font-medium text-[var(--color-text-secondary)] mb-1 uppercase tracking-wider">
                    Currency
                  </label>
                  <select {...register('currency')} className="input-themed">
                    <option value="USD">USD ($)</option>
                    <option value="EUR">EUR</option>
                    <option value="GBP">GBP</option>
                    <option value="CAD">CAD</option>
                    <option value="AUD">AUD</option>
                    <option value="JPY">JPY</option>
                  </select>
                </div>
                <div>
                  <label className="block text-xs font-medium text-[var(--color-text-secondary)] mb-1 uppercase tracking-wider">
                    Opening Balance
                  </label>
                  <input
                    {...register('opening_balance', { valueAsNumber: true })}
                    type="number"
                    step="0.01"
                    className="input-themed"
                  />
                </div>
              </div>
              <div>
                <label className="block text-xs font-medium text-[var(--color-text-secondary)] mb-1 uppercase tracking-wider">
                  Notes
                </label>
                <textarea {...register('notes')} rows={2} className="input-themed" />
              </div>
              <div className="flex justify-end gap-3 pt-2">
                <button
                  type="button"
                  onClick={() => {
                    setShowAddModal(false);
                    reset();
                  }}
                  className="px-4 py-2 text-[var(--color-text-secondary)] border border-[var(--color-border)] text-sm font-medium rounded-xl hover:bg-[var(--color-primary-subtle)] transition-colors"
                >
                  Cancel
                </button>
                <button type="submit" disabled={isSubmitting} className="btn-primary">
                  {isSubmitting ? 'Creating...' : 'Create Account'}
                </button>
              </div>
            </form>
          </div>
        </div>
      )}

      {/* Delete confirmation modal — requires typing the account name */}
      {deleteTarget && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm p-4"
          role="dialog"
          aria-modal="true"
          aria-labelledby="delete-account-title"
        >
          <div className="bg-[var(--color-surface)] border border-red-500/40 rounded-2xl shadow-xl max-w-md w-full p-6">
            <h3 id="delete-account-title" className="text-lg font-semibold text-red-400 mb-2">
              Delete “{deleteTarget.name}”?
            </h3>
            <p className="text-sm text-[var(--color-text-secondary)] mb-4">
              This permanently deletes the account,{' '}
              <strong className="text-[var(--color-text)]">
                {deleteTarget.transaction_count.toLocaleString()}
              </strong>{' '}
              transaction{deleteTarget.transaction_count === 1 ? '' : 's'}, every upload, and all
              associated stored files. This cannot be undone.
            </p>
            <label className="block text-xs font-medium text-[var(--color-text-secondary)] uppercase tracking-wider mb-1">
              Type <span className="font-mono text-[var(--color-text)]">{deleteTarget.name}</span>{' '}
              to confirm
            </label>
            <input
              type="text"
              value={deleteConfirmText}
              onChange={(e) => setDeleteConfirmText(e.target.value)}
              disabled={deleting}
              className="input-themed"
              autoFocus
            />
            {deleteError && (
              <p className="mt-2 text-sm text-red-400" role="alert">
                {deleteError}
              </p>
            )}
            <div className="mt-5 flex justify-end gap-2">
              <button
                onClick={() => setDeleteTarget(null)}
                disabled={deleting}
                className="px-4 py-2 rounded-xl border border-[var(--color-border)] text-[var(--color-text)] text-sm hover:bg-[var(--color-primary-subtle)] transition-colors disabled:opacity-50"
              >
                Cancel
              </button>
              <button
                onClick={() => void handleDeleteAccount()}
                disabled={deleting || deleteConfirmText !== deleteTarget.name}
                className="px-4 py-2 rounded-xl bg-red-600 hover:bg-red-700 text-white text-sm font-medium transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
              >
                {deleting ? 'Deleting…' : 'Permanently delete'}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
