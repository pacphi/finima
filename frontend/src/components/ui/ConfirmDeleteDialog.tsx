import { useEffect, useId, useState, type ReactNode } from 'react';

export interface ConfirmDeleteDialogProps {
  /** Name of the entity being deleted — shown in the title and required as
   *  the confirmation text (case-sensitive exact match). */
  entityName: string;
  /** Warning copy describing the blast radius of the delete. Rendered below
   *  the title. */
  warning: ReactNode;
  /** Invoked when the user confirms. Parent is responsible for the actual
   *  delete request and for flipping `loading` / `error` / unmounting. */
  onConfirm: () => void;
  /** Invoked when the user cancels (Cancel button or Esc). Ignored while
   *  `loading` is true. */
  onCancel: () => void;
  /** Disables inputs and swaps the confirm button label to the busy state. */
  loading?: boolean;
  /** If set, shown in red below the input. */
  error?: string | null;
  /** Overrides the default `Delete "{entityName}"?` title. */
  title?: string;
  /** Overrides the default "Permanently delete" confirm label. */
  confirmLabel?: string;
}

/** Shared "type the name to confirm" destructive-action modal.
 *
 *  Used for deleting portfolios and accounts. Keeps the confirm button
 *  disabled until the typed text exactly matches `entityName` (case- and
 *  whitespace-sensitive). Name is rendered with `textTransform: none` so
 *  the user always sees the original casing regardless of surrounding
 *  uppercase labels.
 */
export function ConfirmDeleteDialog({
  entityName,
  warning,
  onConfirm,
  onCancel,
  loading = false,
  error = null,
  title,
  confirmLabel = 'Permanently delete',
}: ConfirmDeleteDialogProps) {
  const titleId = useId();
  const [confirmText, setConfirmText] = useState('');

  // Esc-to-cancel. Ignored while loading so a mid-flight delete can't be
  // dismissed into an inconsistent UI state. Parent controls mount/unmount,
  // so this listener is only active while the dialog is visible.
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && !loading) onCancel();
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [loading, onCancel]);

  const matches = confirmText === entityName;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm p-4"
      role="dialog"
      aria-modal="true"
      aria-labelledby={titleId}
    >
      <div className="bg-[var(--color-card)] border border-[var(--color-border)] rounded-2xl shadow-xl max-w-md w-full p-6">
        <h3 id={titleId} className="text-lg font-semibold text-red-400 mb-2">
          {title ?? <>Delete “{entityName}”?</>}
        </h3>
        <div className="text-sm text-[var(--color-text-secondary)] mb-4">{warning}</div>
        <label className="block text-xs font-medium text-[var(--color-text-secondary)] mb-1">
          <span className="uppercase tracking-wider">Type </span>
          <span className="font-mono text-[var(--color-text)]" style={{ textTransform: 'none' }}>
            {entityName}
          </span>
          <span className="uppercase tracking-wider"> to confirm</span>
        </label>
        <input
          type="text"
          value={confirmText}
          onChange={(e) => setConfirmText(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter' && matches && !loading) {
              e.preventDefault();
              onConfirm();
            }
          }}
          disabled={loading}
          className="w-full px-3 py-2 rounded-lg border border-[var(--color-border)] bg-[var(--color-surface)] text-[var(--color-text)] text-sm focus:outline-none focus:ring-2 focus:ring-red-500/50"
          autoFocus
        />
        {error && (
          <p className="mt-2 text-sm text-red-400" role="alert">
            {error}
          </p>
        )}
        <div className="mt-5 flex justify-end gap-2">
          <button
            type="button"
            onClick={onCancel}
            disabled={loading}
            className="px-4 py-2 rounded-lg border border-[var(--color-border)] text-[var(--color-text)] text-sm hover:bg-[var(--color-surface)] transition-colors disabled:opacity-50"
          >
            Cancel
          </button>
          <button
            type="button"
            onClick={onConfirm}
            disabled={loading || !matches}
            className="px-4 py-2 rounded-lg bg-red-600 hover:bg-red-700 text-white text-sm font-medium transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          >
            {loading ? 'Deleting…' : confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
}
