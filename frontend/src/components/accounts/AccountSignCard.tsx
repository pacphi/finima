import { useState } from 'react';
import type { Account, SignConvention } from '@/types/models';
import { isLiabilityAccountType } from '@/types/models';

interface AccountSignCardProps {
  account: Account;
  /** Called with the new convention (or `null` to reset). The parent
   *  is expected to PUT /api/accounts/:id/sign-override and refresh
   *  the account. */
  onChange: (convention: SignConvention | null) => Promise<void> | void;
}

const LABEL: Record<SignConvention, string> = {
  positive_means_inflow: 'Positive amounts are money in (deposits, payments received)',
  positive_means_outflow: 'Positive amounts are money out (purchases, charges)',
};

/** Per-account sign-convention override card. End users use the
 *  Flip button when an import looks reversed (purchases showing
 *  as income, or vice-versa). Overrides the institution YAML rule
 *  and any autodetection. See ADR-018. */
export function AccountSignCard({ account, onChange }: AccountSignCardProps) {
  const current = account.sign_convention_override;
  const isExplicit = current !== null;
  const [busy, setBusy] = useState(false);

  const handleFlip = async () => {
    setBusy(true);
    try {
      // Flip relative to the *effective* convention, not just the
      // per-account override. When the server hasn't told us the
      // effective convention (older response shape), fall back to
      // the account-type default (credit cards -> outflow, assets
      // -> inflow). Either way we pin to the opposite so the click
      // produces a visible change.
      const effective: SignConvention =
        current ??
        account.effective_sign_convention ??
        (isLiabilityAccountType(account.account_type)
          ? 'positive_means_outflow'
          : 'positive_means_inflow');
      const next: SignConvention =
        effective === 'positive_means_inflow' ? 'positive_means_outflow' : 'positive_means_inflow';
      await onChange(next);
    } finally {
      setBusy(false);
    }
  };

  const handleReset = async () => {
    setBusy(true);
    try {
      await onChange(null);
    } finally {
      setBusy(false);
    }
  };

  return (
    <section
      className="rounded-lg border border-[var(--color-border)] bg-[var(--color-card)] p-4"
      data-testid="account-sign-card"
    >
      <div className="flex items-start justify-between gap-3">
        <div>
          <h4 className="text-xs font-semibold uppercase tracking-wider text-[var(--color-text-secondary)]">
            Amount Sign
          </h4>
          <p className="mt-1 text-sm text-[var(--color-text)]">
            {isExplicit ? LABEL[current] : 'Automatically detected from imported transactions.'}
          </p>
          <p className="mt-1 text-xs text-[var(--color-text-secondary)]">
            Use this only if your imports look reversed (purchases appear as income, or payments
            appear as charges).
          </p>
        </div>
        <div className="flex shrink-0 flex-col gap-2">
          <button
            type="button"
            onClick={() => void handleFlip()}
            disabled={busy}
            className="rounded border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-1.5 text-xs font-medium text-[var(--color-text)] hover:bg-[var(--color-bg-secondary)] disabled:opacity-50"
          >
            {busy ? 'Working…' : 'Flip this account'}
          </button>
          {isExplicit && (
            <button
              type="button"
              onClick={() => void handleReset()}
              disabled={busy}
              className="rounded px-3 py-1.5 text-xs text-[var(--color-text-secondary)] hover:bg-[var(--color-bg-secondary)] hover:text-[var(--color-text)] disabled:opacity-50"
            >
              Reset to auto
            </button>
          )}
        </div>
      </div>
    </section>
  );
}
