import { useState } from 'react';
import { useApi } from '@/hooks/useApi';
import { createUploadApi } from '@/api/uploads';
import { useFocusTrap } from '@/hooks/useFocusTrap';
import type { UploadPreview } from '@/types/models';
import { COLUMN_MAPPING_OPTIONS } from '@/types/models';

interface ColumnMappingModalProps {
  preview: UploadPreview;
  uploadId: string;
  onComplete: () => void;
  onCancel: () => void;
}

const AUTO_MAPPED_FORMATS = new Set(['ofx', 'qfx', 'qbo', 'qif']);

export function ColumnMappingModal({
  preview,
  uploadId,
  onComplete,
  onCancel,
}: ColumnMappingModalProps) {
  const api = useApi();
  const uploadApi = createUploadApi(api);
  const isAutoMapped = AUTO_MAPPED_FORMATS.has(preview.file_format);
  const trapRef = useFocusTrap<HTMLDivElement>(true, onCancel);

  const [mapping, setMapping] = useState<Record<string, string>>(() => {
    return { ...preview.inferred_mapping };
  });
  const [skipDuplicates, setSkipDuplicates] = useState(true);
  const [importing, setImporting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const previewRows = preview.rows.slice(0, 5);

  const handleMappingChange = (header: string, value: string) => {
    setMapping((prev) => ({ ...prev, [header]: value }));
  };

  const handleConfirm = async () => {
    // Validate required mappings before submitting.
    const targets = Object.values(mapping);
    const hasDate = targets.includes('Date');
    const hasAmount = targets.includes('Amount');
    const hasDebit = targets.includes('Debit');
    const hasCredit = targets.includes('Credit');
    const hasDescription = targets.includes('Description');

    if (!hasDate) {
      setError('Please map a column to "Date".');
      return;
    }
    if (!hasDescription) {
      setError('Please map a column to "Description".');
      return;
    }
    if (!hasAmount && !(hasDebit && hasCredit)) {
      setError('Please map a column to "Amount", or map both "Debit" and "Credit" columns.');
      return;
    }
    if (hasAmount && (hasDebit || hasCredit)) {
      setError('Map either "Amount" or "Debit"/"Credit" — not both.');
      return;
    }
    if ((hasDebit && !hasCredit) || (hasCredit && !hasDebit)) {
      setError('Both "Debit" and "Credit" columns must be mapped together.');
      return;
    }

    setImporting(true);
    setError(null);
    try {
      await uploadApi.confirmUpload(uploadId, {
        mapping,
        skip_duplicates: skipDuplicates,
        date_format: preview.date_format ?? undefined,
      });
      onComplete();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Import failed');
    } finally {
      setImporting(false);
    }
  };

  const modalTitle = isAutoMapped ? 'Confirm Import' : 'Column Mapping';

  return (
    <div
      ref={trapRef}
      role="dialog"
      aria-modal="true"
      aria-labelledby="column-mapping-title"
      className="space-y-4"
      tabIndex={-1}
    >
      <div className="flex items-center justify-between">
        <h3 id="column-mapping-title" className="text-lg font-semibold text-[var(--color-text)]">
          {modalTitle}
        </h3>
        <button
          onClick={onCancel}
          className="text-sm text-[var(--color-text-secondary)] hover:text-[var(--color-text)]"
          aria-label="Cancel import"
        >
          Cancel
        </button>
      </div>

      <div className="text-sm text-[var(--color-text-secondary)]">
        <span className="font-medium">{preview.file_name}</span>
        <span className="ml-2 inline-block px-1.5 py-0.5 badge-primary rounded text-xs font-medium uppercase">
          {preview.file_format}
        </span>
        <span className="ml-2">{preview.row_count} rows</span>
      </div>

      {!isAutoMapped && (
        <div className="border border-[var(--color-border)] rounded-lg overflow-hidden">
          <table className="w-full text-sm" aria-label="Column mapping configuration">
            <thead className="bg-[var(--color-surface)]">
              <tr>
                <th
                  scope="col"
                  className="text-left px-3 py-2 text-xs font-medium text-[var(--color-text-secondary)] uppercase"
                >
                  File Column
                </th>
                <th
                  scope="col"
                  className="text-left px-3 py-2 text-xs font-medium text-[var(--color-text-secondary)] uppercase"
                >
                  Maps To
                </th>
              </tr>
            </thead>
            <tbody className="divide-y divide-[var(--color-border)]">
              {preview.headers.map((header) => (
                <tr key={header}>
                  <td className="px-3 py-2 font-mono text-[var(--color-text)]">{header}</td>
                  <td className="px-3 py-2">
                    <label htmlFor={`mapping-${header}`} className="sr-only">
                      Map column "{header}" to
                    </label>
                    <select
                      id={`mapping-${header}`}
                      value={mapping[header] ?? '-- Skip --'}
                      onChange={(e) => handleMappingChange(header, e.target.value)}
                      className="block w-full rounded border border-[var(--color-input-border)] px-2 py-1 text-sm focus:border-[var(--color-primary)] focus:ring-1 focus:ring-[var(--color-primary)]"
                    >
                      {COLUMN_MAPPING_OPTIONS.map((opt) => (
                        <option key={opt} value={opt}>
                          {opt}
                        </option>
                      ))}
                    </select>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {/* Preview table */}
      <div>
        <h4 className="text-sm font-medium text-[var(--color-text-secondary)] mb-2">
          Preview (first {previewRows.length} rows)
        </h4>
        <div className="border border-[var(--color-border)] rounded-lg overflow-x-auto">
          <table className="w-full text-sm" aria-label="Data preview">
            <thead className="bg-[var(--color-surface)]">
              <tr>
                {preview.headers.map((h) => (
                  <th
                    key={h}
                    scope="col"
                    className="text-left px-3 py-2 text-xs font-medium text-[var(--color-text-secondary)] whitespace-nowrap"
                  >
                    {h}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody className="divide-y divide-[var(--color-border)]">
              {previewRows.map((row, i) => (
                <tr key={i} className={i % 2 === 0 ? '' : 'bg-[var(--color-surface)]'}>
                  {row.map((cell, j) => (
                    <td key={j} className="px-3 py-2 text-[var(--color-text)] whitespace-nowrap">
                      {cell}
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>

      {preview.date_format && (
        <p className="text-sm text-[var(--color-text-secondary)]">
          Date format detected: <span className="font-mono font-medium">{preview.date_format}</span>
        </p>
      )}

      <label className="flex items-center gap-2 text-sm text-[var(--color-text)]">
        <input
          type="checkbox"
          checked={skipDuplicates}
          onChange={(e) => setSkipDuplicates(e.target.checked)}
          className="rounded border-[var(--color-input-border)]"
        />
        Skip duplicate transactions (by date + amount + description)
      </label>

      {error && (
        <div
          className="p-3 bg-red-50 border border-red-200 rounded-lg text-sm text-red-700"
          role="alert"
          aria-live="assertive"
        >
          {error}
        </div>
      )}

      <div className="flex justify-end gap-3">
        <button
          onClick={onCancel}
          className="px-4 py-2 text-[var(--color-text-secondary)] border border-[var(--color-border)] text-sm font-medium rounded-lg hover:bg-[var(--color-primary-subtle)] transition-colors"
        >
          Cancel
        </button>
        <button
          onClick={handleConfirm}
          disabled={importing}
          className="px-4 py-2 btn-primary text-sm font-medium rounded-lg disabled:opacity-50 transition-colors"
        >
          {importing ? 'Importing...' : `Import ${preview.row_count} rows`}
        </button>
      </div>
    </div>
  );
}
