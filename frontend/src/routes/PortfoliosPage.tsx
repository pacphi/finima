import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useApi } from '@/hooks/useApi';
import { createPortfolioApi } from '@/api/portfolios';
import { usePortfolioStore } from '@/stores/portfolioStore';
import { formatCurrency } from '@/utils/format';
import { ACCOUNT_TYPE_LABELS, ACCOUNT_TYPE_ICONS } from '@/types/models';
import type { Portfolio, Account } from '@/types/models';

export function PortfoliosPage() {
  const navigate = useNavigate();
  const api = useApi();
  const portfolioApi = createPortfolioApi(api);

  const portfolios = usePortfolioStore((s) => s.portfolios);
  const activePortfolioId = usePortfolioStore((s) => s.activePortfolioId);
  const accounts = usePortfolioStore((s) => s.accounts);
  const selectPortfolio = usePortfolioStore((s) => s.selectPortfolio);
  const setPortfolios = usePortfolioStore((s) => s.setPortfolios);
  const addPortfolio = usePortfolioStore((s) => s.addPortfolio);

  const [showModal, setShowModal] = useState(false);
  const [editingPortfolio, setEditingPortfolio] = useState<Portfolio | null>(null);
  const [name, setName] = useState('');
  const [description, setDescription] = useState('');
  const [saving, setSaving] = useState(false);

  const openModal = (portfolio?: Portfolio) => {
    if (portfolio) {
      setEditingPortfolio(portfolio);
      setName(portfolio.name);
      setDescription(portfolio.description ?? '');
    } else {
      setEditingPortfolio(null);
      setName('');
      setDescription('');
    }
    setShowModal(true);
  };

  const handleSave = async () => {
    if (!name.trim()) return;
    setSaving(true);
    try {
      if (editingPortfolio) {
        const updated = await portfolioApi.updatePortfolio(editingPortfolio.id, {
          name: name.trim(),
          description: description.trim() || undefined,
        });
        setPortfolios(portfolios.map((p) => (p.id === updated.id ? updated : p)));
      } else {
        const created = await portfolioApi.createPortfolio({
          name: name.trim(),
          description: description.trim() || undefined,
        });
        addPortfolio(created);
      }
      setShowModal(false);
    } catch (err) {
      console.error('Failed to save portfolio:', err);
    } finally {
      setSaving(false);
    }
  };

  const accountsForPortfolio = (portfolioId: string): Account[] =>
    activePortfolioId === portfolioId ? accounts : [];

  return (
    <div className="p-6 lg:p-8">
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-2xl font-bold text-[var(--color-text)] tracking-tight">Portfolios</h1>
        <button onClick={() => openModal()} className="btn-primary">
          + New Portfolio
        </button>
      </div>

      {portfolios.length === 0 ? (
        <div className="text-center py-16">
          <p className="text-[var(--color-text-secondary)] mb-2">No portfolios yet</p>
          <p className="text-sm text-[var(--color-text-secondary)] mb-4">
            Create a portfolio to organize your accounts.
          </p>
          <button onClick={() => openModal()} className="btn-primary">
            Create Your First Portfolio
          </button>
        </div>
      ) : (
        <div className="space-y-4">
          {portfolios.map((portfolio) => {
            const isActive = portfolio.id === activePortfolioId;
            const portfolioAccounts = accountsForPortfolio(portfolio.id);

            return (
              <div
                key={portfolio.id}
                className={`bg-[var(--color-card)] backdrop-blur-sm rounded-2xl border p-5 transition-all ${
                  isActive
                    ? 'border-[var(--color-primary)] shadow-[0_0_0_1px_var(--color-primary)]'
                    : 'border-[var(--color-border)] hover:border-[var(--color-primary-muted)]'
                }`}
              >
                {/* Portfolio header */}
                <div className="flex items-center justify-between mb-3">
                  <div className="flex items-center gap-3">
                    <div className="w-10 h-10 rounded-xl bg-gradient-to-br from-[var(--color-primary)] to-[var(--color-primary-hover)] flex items-center justify-center text-white text-sm font-bold shadow-sm">
                      {portfolio.name.charAt(0).toUpperCase()}
                    </div>
                    <div>
                      <h2 className="font-semibold text-[var(--color-text)]">{portfolio.name}</h2>
                      {portfolio.description && (
                        <p className="text-xs text-[var(--color-text-secondary)]">
                          {portfolio.description}
                        </p>
                      )}
                    </div>
                    {isActive && (
                      <span className="px-2 py-0.5 text-xs font-medium rounded-full bg-[var(--color-primary)]/15 text-[var(--color-primary)]">
                        Active
                      </span>
                    )}
                  </div>
                  <div className="flex items-center gap-2">
                    {!isActive && (
                      <button
                        onClick={() => selectPortfolio(portfolio.id)}
                        className="px-3 py-1.5 text-sm font-medium text-[var(--color-primary)] border border-[var(--color-primary)] rounded-xl hover:bg-[var(--color-primary)] hover:text-white transition-colors"
                      >
                        Switch To
                      </button>
                    )}
                    <button
                      onClick={() => openModal(portfolio)}
                      className="px-3 py-1.5 text-sm text-[var(--color-text-secondary)] border border-[var(--color-border)] rounded-xl hover:bg-[var(--color-primary-subtle)] hover:text-[var(--color-text)] transition-colors"
                    >
                      Edit
                    </button>
                  </div>
                </div>

                {/* Accounts list for active portfolio */}
                {isActive && portfolioAccounts.length > 0 && (
                  <div className="mt-3 pt-3 border-t border-[var(--color-border)]">
                    <div className="flex items-center justify-between mb-2">
                      <h3 className="text-xs font-medium text-[var(--color-text-secondary)] uppercase tracking-wider">
                        Accounts ({portfolioAccounts.length})
                      </h3>
                      <button
                        onClick={() => navigate('/accounts')}
                        className="text-xs text-[var(--color-primary)] hover:underline"
                      >
                        Manage accounts
                      </button>
                    </div>
                    <div className="space-y-1.5">
                      {portfolioAccounts.map((account) => (
                        <button
                          key={account.id}
                          onClick={() => navigate(`/accounts/${account.id}`)}
                          className="w-full text-left flex items-center justify-between px-3 py-2 rounded-lg hover:bg-[var(--color-surface)] transition-colors"
                        >
                          <div className="flex items-center gap-2">
                            <span className="text-sm">
                              {ACCOUNT_TYPE_ICONS[account.account_type]}
                            </span>
                            <span className="text-sm text-[var(--color-text)]">{account.name}</span>
                            <span className="text-xs text-[var(--color-text-secondary)]">
                              {ACCOUNT_TYPE_LABELS[account.account_type]}
                            </span>
                          </div>
                          <span
                            className={`text-sm font-medium ${
                              account.current_balance >= 0
                                ? 'text-[var(--color-text)]'
                                : 'text-[var(--color-error)]'
                            }`}
                          >
                            {formatCurrency(account.current_balance)}
                          </span>
                        </button>
                      ))}
                    </div>
                  </div>
                )}

                {isActive && portfolioAccounts.length === 0 && (
                  <div className="mt-3 pt-3 border-t border-[var(--color-border)]">
                    <p className="text-sm text-[var(--color-text-secondary)]">
                      No accounts yet.{' '}
                      <button
                        onClick={() => navigate('/accounts')}
                        className="text-[var(--color-primary)] hover:underline"
                      >
                        Add an account
                      </button>
                    </p>
                  </div>
                )}

                {!isActive && (
                  <p className="text-xs text-[var(--color-text-secondary)] mt-1">
                    Switch to this portfolio to see its accounts.
                  </p>
                )}
              </div>
            );
          })}
        </div>
      )}

      {/* Portfolio Modal */}
      {showModal && (
        <div className="fixed inset-0 z-50 flex items-center justify-center">
          <div
            className="absolute inset-0 bg-black/60 backdrop-blur-sm"
            onClick={() => setShowModal(false)}
            aria-hidden="true"
          />
          <div
            role="dialog"
            aria-modal="true"
            aria-labelledby="portfolio-modal-title"
            className="relative bg-[var(--color-surface)] border border-[var(--color-border)] rounded-2xl shadow-xl w-full max-w-md p-6 z-10"
          >
            <h2
              id="portfolio-modal-title"
              className="text-lg font-semibold text-[var(--color-text)] mb-4"
            >
              {editingPortfolio ? 'Edit Portfolio' : 'New Portfolio'}
            </h2>
            <div className="space-y-4">
              <div>
                <label className="block text-xs font-medium text-[var(--color-text-secondary)] mb-1 uppercase tracking-wider">
                  Name
                </label>
                <input
                  type="text"
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  className="input-themed"
                  placeholder="My Portfolio"
                  autoFocus
                />
              </div>
              <div>
                <label className="block text-xs font-medium text-[var(--color-text-secondary)] mb-1 uppercase tracking-wider">
                  Description (optional)
                </label>
                <textarea
                  value={description}
                  onChange={(e) => setDescription(e.target.value)}
                  rows={2}
                  className="input-themed"
                  placeholder="Family finances, investment tracking, etc."
                />
              </div>
            </div>
            <div className="flex justify-end gap-3 pt-4">
              <button
                type="button"
                onClick={() => setShowModal(false)}
                className="px-4 py-2 text-[var(--color-text-secondary)] border border-[var(--color-border)] text-sm font-medium rounded-xl hover:bg-[var(--color-primary-subtle)] transition-colors"
              >
                Cancel
              </button>
              <button
                onClick={() => void handleSave()}
                disabled={saving || !name.trim()}
                className="btn-primary"
              >
                {saving ? 'Saving...' : editingPortfolio ? 'Update' : 'Create'}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
