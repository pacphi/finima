import { useState, useEffect, useCallback, useMemo, useRef } from 'react';
import { useApi } from '@/hooks/useApi';
import { createCategoryApi, type CategoryEntry, type SubcategoryEntry } from '@/api/categories';
import { invalidateCategoryCache } from '@/hooks/useCategories';

type FilterMode = 'all' | 'system' | 'custom';

function normalizeKey(raw: string): string {
  return raw.trim().toLowerCase().replace(/\s+/g, '_');
}

function isValidKey(k: string): boolean {
  return /^[a-z0-9_]+$/.test(k);
}

export function CategoriesTab() {
  const api = useApi();
  const categoryApi = useMemo(() => createCategoryApi(api), [api]);

  const [categories, setCategories] = useState<CategoryEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [successMsg, setSuccessMsg] = useState<string | null>(null);

  // Parent Add form
  const [newParentKey, setNewParentKey] = useState('');
  const [newParentLabel, setNewParentLabel] = useState('');

  // Per-parent subcategory add drafts: { [parentKey]: { key, label } }
  const [subDrafts, setSubDrafts] = useState<Record<string, { key: string; label: string }>>({});

  // Expansion
  const [expanded, setExpanded] = useState<Set<string>>(new Set());

  // Edit state: we key by `${parent ?? ''}:${key}` to disambiguate.
  type EditTarget = { parentKey: string | null; key: string; label: string };
  const [editing, setEditing] = useState<EditTarget | null>(null);
  const [editLabel, setEditLabel] = useState('');

  // Row menu open state
  const [openMenu, setOpenMenu] = useState<string | null>(null);

  // Move-to dialog state
  const [moveTarget, setMoveTarget] = useState<SubcategoryEntry | null>(null);
  const [moveParent, setMoveParent] = useState<string>('');
  const [moveCurrentParent, setMoveCurrentParent] = useState<string>('');

  // Search + filter
  const [search, setSearch] = useState('');
  const [filter, setFilter] = useState<FilterMode>('all');

  // Import dialog
  const [importOpen, setImportOpen] = useState(false);
  const [importText, setImportText] = useState('');
  const [importReplace, setImportReplace] = useState(false);
  const [importBusy, setImportBusy] = useState(false);

  const fetchCategories = useCallback(async () => {
    try {
      const data = await categoryApi.listCategories();
      const sorted = [...data]
        .sort((a, b) => a.label.localeCompare(b.label))
        .map((cat) => ({
          ...cat,
          subcategories: cat.subcategories
            ? [...cat.subcategories].sort((a, b) => a.label.localeCompare(b.label))
            : [],
        }));
      setCategories(sorted);
    } catch (err) {
      console.error('Failed to load categories:', err);
    } finally {
      setLoading(false);
    }
  }, [categoryApi]);

  useEffect(() => {
    let ignore = false;
    (async () => {
      try {
        const data = await categoryApi.listCategories();
        if (ignore) return;
        const sorted = [...data]
          .sort((a, b) => a.label.localeCompare(b.label))
          .map((cat) => ({
            ...cat,
            subcategories: cat.subcategories
              ? [...cat.subcategories].sort((a, b) => a.label.localeCompare(b.label))
              : [],
          }));
        setCategories(sorted);
      } catch (err) {
        if (!ignore) console.error('Failed to load categories:', err);
      } finally {
        if (!ignore) setLoading(false);
      }
    })();
    return () => {
      ignore = true;
    };
  }, [categoryApi]);

  // Close row menu on outside click
  const menuRef = useRef<HTMLDivElement | null>(null);
  useEffect(() => {
    function handler(e: MouseEvent) {
      if (!openMenu) return;
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) setOpenMenu(null);
    }
    document.addEventListener('mousedown', handler);
    return () => document.removeEventListener('mousedown', handler);
  }, [openMenu]);

  // ── Filters ─────────────────────────────────────────────────────────
  const filtered = useMemo(() => {
    const q = search.trim().toLowerCase();
    const matches = (key: string, label: string) =>
      !q || key.toLowerCase().includes(q) || label.toLowerCase().includes(q);

    return categories
      .map((cat) => {
        const subs = (cat.subcategories ?? []).filter((s) => {
          if (filter === 'system' && !s.is_system) return false;
          if (filter === 'custom' && s.is_system) return false;
          return matches(s.key, s.label);
        });
        return { ...cat, subcategories: subs, _subHit: subs.length > 0 };
      })
      .filter((cat) => {
        const parentOk =
          (filter === 'all' ||
            (filter === 'system' && cat.is_system) ||
            (filter === 'custom' && !cat.is_system)) &&
          matches(cat.key, cat.label);
        return parentOk || cat._subHit;
      });
  }, [categories, search, filter]);

  // Auto-expand parents whose subs matched the search. Derived during render so
  // we don't duplicate state into a separate `expanded` set via an effect.
  const effectiveExpanded = useMemo(() => {
    if (!search.trim()) return expanded;
    const next = new Set(expanded);
    for (const c of filtered) {
      if ((c as CategoryEntry & { _subHit?: boolean })._subHit) next.add(c.key);
    }
    return next;
  }, [search, filtered, expanded]);

  // ── Helpers ─────────────────────────────────────────────────────────
  const showSuccess = (msg: string) => {
    setSuccessMsg(msg);
    setTimeout(() => setSuccessMsg(null), 3000);
  };

  const toggleExpand = (key: string) => {
    setExpanded((prev) => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  };

  // ── CRUD ────────────────────────────────────────────────────────────
  const handleAddParent = async () => {
    setError(null);
    const key = normalizeKey(newParentKey);
    const label = newParentLabel.trim();
    if (!key || !label) return setError('Both key and label are required');
    if (!isValidKey(key))
      return setError('Key must be lowercase letters, numbers, and underscores only');
    try {
      await categoryApi.createCategory(key, label);
      invalidateCategoryCache();
      setNewParentKey('');
      setNewParentLabel('');
      showSuccess(`Added "${label}"`);
      void fetchCategories();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to add category');
    }
  };

  const handleAddSub = async (parentKey: string) => {
    setError(null);
    const draft = subDrafts[parentKey] ?? { key: '', label: '' };
    const key = normalizeKey(draft.key);
    const label = draft.label.trim();
    if (!key || !label) return setError('Both key and label are required');
    if (!isValidKey(key))
      return setError('Key must be lowercase letters, numbers, and underscores only');
    try {
      await categoryApi.createCategory(key, label, parentKey);
      invalidateCategoryCache();
      setSubDrafts((prev) => ({ ...prev, [parentKey]: { key: '', label: '' } }));
      showSuccess(`Added "${label}"`);
      void fetchCategories();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to add subcategory');
    }
  };

  const startEdit = (parentKey: string | null, key: string, label: string) => {
    setEditing({ parentKey, key, label });
    setEditLabel(label);
    setOpenMenu(null);
  };

  const handleUpdate = async () => {
    if (!editing) return;
    setError(null);
    const label = editLabel.trim();
    if (!label) return setError('Label is required');
    try {
      await categoryApi.updateCategory(editing.key, label, editing.parentKey ?? undefined);
      invalidateCategoryCache();
      setEditing(null);
      setEditLabel('');
      showSuccess(`Updated "${label}"`);
      void fetchCategories();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to update category');
    }
  };

  const handleDelete = async (key: string, label: string) => {
    setError(null);
    try {
      await categoryApi.deleteCategory(key);
      invalidateCategoryCache();
      showSuccess(`Deleted "${label}"`);
      setOpenMenu(null);
      void fetchCategories();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to delete category');
    }
  };

  const openMove = (parentKey: string, sub: SubcategoryEntry) => {
    setMoveTarget(sub);
    setMoveParent(parentKey);
    setMoveCurrentParent(parentKey);
    setOpenMenu(null);
  };

  const handleMove = async () => {
    if (!moveTarget) return;
    setError(null);
    try {
      await categoryApi.updateCategory(moveTarget.key, moveTarget.label, moveParent);
      invalidateCategoryCache();
      showSuccess(`Moved "${moveTarget.label}"`);
      setMoveTarget(null);
      void fetchCategories();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to move subcategory');
    }
  };

  // ── Import / Export ─────────────────────────────────────────────────
  const handleExport = async () => {
    setError(null);
    try {
      const { yaml } = await categoryApi.exportYaml();
      const blob = new Blob([yaml], { type: 'application/x-yaml' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = 'categories.yaml';
      document.body.appendChild(a);
      a.click();
      a.remove();
      URL.revokeObjectURL(url);
      showSuccess('Exported categories.yaml');
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Export failed');
    }
  };

  const handleImport = async () => {
    setError(null);
    setImportBusy(true);
    try {
      const res = await categoryApi.importYaml(importText, importReplace);
      invalidateCategoryCache();
      showSuccess(
        `Imported — created ${res.created}, updated ${res.updated}, removed ${res.removed}`,
      );
      setImportOpen(false);
      setImportText('');
      setImportReplace(false);
      void fetchCategories();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Import failed');
    } finally {
      setImportBusy(false);
    }
  };

  const allParentKeys = useMemo(() => categories.map((c) => c.key), [categories]);

  // ── Render ──────────────────────────────────────────────────────────
  if (loading) {
    return <div className="text-sm text-[var(--color-text-secondary)]">Loading categories…</div>;
  }

  return (
    <div className="space-y-5 max-w-3xl">
      {/* Heading + description */}
      <div>
        <h3 className="text-lg font-medium text-[var(--color-text)] mb-1">
          Transaction Categories
        </h3>
        <p className="text-sm text-[var(--color-text-secondary)]">
          Manage the categories and subcategories used to classify transactions. System entries can
          be relabeled or reorganized but not deleted. Custom entries can be added, edited, moved,
          and removed.
        </p>
      </div>

      {/* Error / success banners */}
      {error && (
        <div className="p-3 text-sm bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg text-red-700 dark:text-red-400">
          {error}
        </div>
      )}
      {successMsg && (
        <div
          className="p-3 text-sm bg-[var(--color-primary-subtle)] border border-[var(--color-primary-muted)] rounded-lg text-[var(--color-primary)]"
          role="status"
        >
          {successMsg}
        </div>
      )}

      {/* Toolbar */}
      <div className="flex flex-wrap gap-2 items-center">
        <input
          type="search"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          placeholder="Search categories & subcategories…"
          className="flex-1 min-w-[200px] px-3 py-2 text-sm border border-[var(--color-input-border)] bg-[var(--color-input-bg)] text-[var(--color-text)] rounded-lg focus:border-[var(--color-primary)] focus:ring-1 focus:ring-[var(--color-primary)]"
        />
        <div
          className="flex rounded-lg border border-[var(--color-border)] overflow-hidden text-xs"
          role="group"
          aria-label="Filter by type"
        >
          {(['all', 'system', 'custom'] as FilterMode[]).map((m) => (
            <button
              key={m}
              onClick={() => setFilter(m)}
              className={`px-3 py-2 capitalize transition-colors ${
                filter === m
                  ? 'bg-[var(--color-primary)] text-white'
                  : 'bg-[var(--color-surface)] text-[var(--color-text-secondary)] hover:text-[var(--color-text)]'
              }`}
            >
              {m}
            </button>
          ))}
        </div>
        <button
          onClick={() => void handleExport()}
          className="px-3 py-2 text-xs border border-[var(--color-border)] text-[var(--color-text)] rounded-lg hover:bg-[var(--color-surface)]"
        >
          Export YAML
        </button>
        <button
          onClick={() => setImportOpen(true)}
          className="px-3 py-2 text-xs border border-[var(--color-border)] text-[var(--color-text)] rounded-lg hover:bg-[var(--color-surface)]"
        >
          Import YAML
        </button>
        <button
          onClick={() =>
            setExpanded(
              expanded.size === categories.length
                ? new Set()
                : new Set(categories.map((c) => c.key)),
            )
          }
          className="px-3 py-2 text-xs border border-[var(--color-border)] text-[var(--color-text)] rounded-lg hover:bg-[var(--color-surface)]"
        >
          {expanded.size === categories.length ? 'Collapse all' : 'Expand all'}
        </button>
      </div>

      {/* Add parent category */}
      <div className="flex gap-2 items-end p-3 border border-dashed border-[var(--color-border)] rounded-lg">
        <div className="flex-1">
          <label className="block text-xs font-medium text-[var(--color-text-secondary)] mb-1">
            New category key
          </label>
          <input
            type="text"
            value={newParentKey}
            onChange={(e) => setNewParentKey(e.target.value)}
            placeholder="e.g. subscriptions"
            className="w-full px-3 py-2 text-sm border border-[var(--color-input-border)] bg-[var(--color-input-bg)] text-[var(--color-text)] rounded-lg focus:border-[var(--color-primary)] focus:ring-1 focus:ring-[var(--color-primary)]"
          />
        </div>
        <div className="flex-1">
          <label className="block text-xs font-medium text-[var(--color-text-secondary)] mb-1">
            New category label
          </label>
          <input
            type="text"
            value={newParentLabel}
            onChange={(e) => setNewParentLabel(e.target.value)}
            placeholder="e.g. Subscriptions"
            onKeyDown={(e) => {
              if (e.key === 'Enter') void handleAddParent();
            }}
            className="w-full px-3 py-2 text-sm border border-[var(--color-input-border)] bg-[var(--color-input-bg)] text-[var(--color-text)] rounded-lg focus:border-[var(--color-primary)] focus:ring-1 focus:ring-[var(--color-primary)]"
          />
        </div>
        <button
          onClick={() => void handleAddParent()}
          className="px-4 py-2 text-sm font-medium bg-[var(--color-primary)] text-white rounded-lg hover:bg-[var(--color-primary-hover)]"
        >
          Add
        </button>
      </div>

      {/* Tree table */}
      <div className="border border-[var(--color-border)] rounded-lg overflow-hidden">
        <table className="w-full text-sm">
          <thead className="bg-[var(--color-surface)] sticky top-0 z-10">
            <tr>
              <th className="w-6"></th>
              <th className="text-left px-3 py-2.5 text-xs font-medium text-[var(--color-text-secondary)] uppercase">
                Key
              </th>
              <th className="text-left px-3 py-2.5 text-xs font-medium text-[var(--color-text-secondary)] uppercase">
                Label
              </th>
              <th className="text-left px-3 py-2.5 text-xs font-medium text-[var(--color-text-secondary)] uppercase">
                Type
              </th>
              <th className="text-right px-3 py-2.5 text-xs font-medium text-[var(--color-text-secondary)] uppercase">
                Actions
              </th>
            </tr>
          </thead>
          <tbody className="divide-y divide-[var(--color-border)]">
            {filtered.map((cat) => {
              const isOpen = effectiveExpanded.has(cat.key);
              const subCount = (cat.subcategories ?? []).length;
              const isEditingParent =
                editing && editing.parentKey === null && editing.key === cat.key;
              return (
                <CategoryRowGroup key={cat.key}>
                  <tr className="hover:bg-[var(--color-surface)] transition-colors">
                    <td className="px-1 text-center">
                      <button
                        onClick={() => toggleExpand(cat.key)}
                        className="w-6 h-6 flex items-center justify-center text-[var(--color-text-secondary)] hover:text-[var(--color-text)]"
                        aria-label={isOpen ? 'Collapse' : 'Expand'}
                        aria-expanded={isOpen}
                      >
                        <svg
                          width="12"
                          height="12"
                          viewBox="0 0 12 12"
                          className={`transition-transform ${isOpen ? 'rotate-90' : ''}`}
                        >
                          <path
                            d="M4 2 L8 6 L4 10"
                            stroke="currentColor"
                            strokeWidth="1.5"
                            fill="none"
                          />
                        </svg>
                      </button>
                    </td>
                    <td className="px-3 py-2.5 font-mono text-xs text-[var(--color-text-secondary)]">
                      {cat.key}
                    </td>
                    <td className="px-3 py-2.5">
                      {isEditingParent ? (
                        <input
                          type="text"
                          value={editLabel}
                          onChange={(e) => setEditLabel(e.target.value)}
                          onKeyDown={(e) => {
                            if (e.key === 'Enter') void handleUpdate();
                            if (e.key === 'Escape') setEditing(null);
                          }}
                          autoFocus
                          className="w-full px-2 py-1 text-sm border border-[var(--color-primary)] bg-[var(--color-input-bg)] text-[var(--color-text)] rounded"
                        />
                      ) : (
                        <span className="text-[var(--color-text)] font-medium">
                          {cat.label}{' '}
                          {subCount > 0 && (
                            <span className="ml-2 text-xs text-[var(--color-text-secondary)]">
                              · {subCount}
                            </span>
                          )}
                        </span>
                      )}
                    </td>
                    <td className="px-3 py-2.5">
                      <TypeBadge isSystem={cat.is_system} />
                    </td>
                    <td className="px-3 py-2.5 text-right">
                      {isEditingParent ? (
                        <InlineActions
                          onSave={() => void handleUpdate()}
                          onCancel={() => setEditing(null)}
                        />
                      ) : (
                        <RowMenu
                          id={`cat:${cat.key}`}
                          open={openMenu === `cat:${cat.key}`}
                          setOpen={(k) => setOpenMenu(k)}
                          menuRef={menuRef}
                          canDelete={!cat.is_system}
                          onEdit={() => startEdit(null, cat.key, cat.label)}
                          onDelete={() => void handleDelete(cat.key, cat.label)}
                        />
                      )}
                    </td>
                  </tr>

                  {isOpen && (
                    <>
                      {(cat.subcategories ?? []).map((sub) => {
                        const isEditingSub =
                          editing && editing.parentKey === cat.key && editing.key === sub.key;
                        return (
                          <tr
                            key={`${cat.key}::${sub.key}`}
                            className="bg-[var(--color-surface)]/40 hover:bg-[var(--color-surface)] transition-colors"
                          >
                            <td></td>
                            <td className="px-3 py-2 font-mono text-xs text-[var(--color-text-secondary)] pl-8 relative">
                              <span className="absolute left-3 top-0 bottom-0 w-px bg-[var(--color-border)]" />
                              <span className="absolute left-3 top-1/2 w-4 h-px bg-[var(--color-border)]" />
                              {sub.key}
                            </td>
                            <td className="px-3 py-2">
                              {isEditingSub ? (
                                <input
                                  type="text"
                                  value={editLabel}
                                  onChange={(e) => setEditLabel(e.target.value)}
                                  onKeyDown={(e) => {
                                    if (e.key === 'Enter') void handleUpdate();
                                    if (e.key === 'Escape') setEditing(null);
                                  }}
                                  autoFocus
                                  className="w-full px-2 py-1 text-sm border border-[var(--color-primary)] bg-[var(--color-input-bg)] text-[var(--color-text)] rounded"
                                />
                              ) : (
                                <span className="text-[var(--color-text)]">{sub.label}</span>
                              )}
                            </td>
                            <td className="px-3 py-2">
                              <TypeBadge isSystem={sub.is_system} />
                            </td>
                            <td className="px-3 py-2 text-right">
                              {isEditingSub ? (
                                <InlineActions
                                  onSave={() => void handleUpdate()}
                                  onCancel={() => setEditing(null)}
                                />
                              ) : (
                                <RowMenu
                                  id={`sub:${cat.key}:${sub.key}`}
                                  open={openMenu === `sub:${cat.key}:${sub.key}`}
                                  setOpen={(k) => setOpenMenu(k)}
                                  menuRef={menuRef}
                                  canDelete={!sub.is_system}
                                  canMove={!sub.is_system}
                                  onEdit={() => startEdit(cat.key, sub.key, sub.label)}
                                  onDelete={() => void handleDelete(sub.key, sub.label)}
                                  onMove={() => openMove(cat.key, sub)}
                                />
                              )}
                            </td>
                          </tr>
                        );
                      })}

                      {/* Inline add-sub form */}
                      <tr className="bg-[var(--color-surface)]/20">
                        <td></td>
                        <td colSpan={4} className="px-3 py-2 pl-8">
                          <div className="flex gap-2 items-center">
                            <span className="text-xs text-[var(--color-text-secondary)] px-2 py-1 rounded-full bg-[var(--color-surface)] whitespace-nowrap">
                              under {cat.label}
                            </span>
                            <input
                              type="text"
                              value={subDrafts[cat.key]?.key ?? ''}
                              onChange={(e) =>
                                setSubDrafts((prev) => ({
                                  ...prev,
                                  [cat.key]: {
                                    key: e.target.value,
                                    label: prev[cat.key]?.label ?? '',
                                  },
                                }))
                              }
                              placeholder="sub key"
                              className="flex-1 min-w-[120px] px-2 py-1.5 text-xs border border-[var(--color-input-border)] bg-[var(--color-input-bg)] text-[var(--color-text)] rounded"
                            />
                            <input
                              type="text"
                              value={subDrafts[cat.key]?.label ?? ''}
                              onChange={(e) =>
                                setSubDrafts((prev) => ({
                                  ...prev,
                                  [cat.key]: {
                                    key: prev[cat.key]?.key ?? '',
                                    label: e.target.value,
                                  },
                                }))
                              }
                              onKeyDown={(e) => {
                                if (e.key === 'Enter') void handleAddSub(cat.key);
                              }}
                              placeholder="Sub label"
                              className="flex-1 min-w-[120px] px-2 py-1.5 text-xs border border-[var(--color-input-border)] bg-[var(--color-input-bg)] text-[var(--color-text)] rounded"
                            />
                            <button
                              onClick={() => void handleAddSub(cat.key)}
                              className="px-3 py-1.5 text-xs font-medium bg-[var(--color-primary)] text-white rounded hover:bg-[var(--color-primary-hover)]"
                            >
                              Add sub
                            </button>
                          </div>
                        </td>
                      </tr>
                    </>
                  )}
                </CategoryRowGroup>
              );
            })}

            {filtered.length === 0 && (
              <tr>
                <td
                  colSpan={5}
                  className="px-3 py-8 text-center text-sm text-[var(--color-text-secondary)]"
                >
                  No categories match your search.
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>

      {/* Move-to dialog */}
      {moveTarget && (
        <Modal onClose={() => setMoveTarget(null)} title={`Move "${moveTarget.label}"`}>
          <p className="text-sm text-[var(--color-text-secondary)] mb-3">
            Move this subcategory to a different parent.
          </p>
          <select
            value={moveParent}
            onChange={(e) => setMoveParent(e.target.value)}
            className="w-full px-3 py-2 text-sm border border-[var(--color-input-border)] bg-[var(--color-input-bg)] text-[var(--color-text)] rounded-lg"
          >
            {allParentKeys.map((k) => (
              <option key={k} value={k}>
                {k}
              </option>
            ))}
          </select>
          <div className="mt-4 flex justify-end gap-2">
            <button
              onClick={() => setMoveTarget(null)}
              className="px-4 py-2 text-sm text-[var(--color-text-secondary)] hover:text-[var(--color-text)]"
            >
              Cancel
            </button>
            <button
              onClick={() => void handleMove()}
              disabled={moveParent === moveCurrentParent}
              className="px-4 py-2 text-sm font-medium bg-[var(--color-primary)] text-white rounded-lg hover:bg-[var(--color-primary-hover)] disabled:opacity-50"
            >
              Move
            </button>
          </div>
        </Modal>
      )}

      {/* Import dialog */}
      {importOpen && (
        <Modal onClose={() => setImportOpen(false)} title="Import categories YAML">
          <p className="text-sm text-[var(--color-text-secondary)] mb-3">
            Paste YAML matching the shape of <code>config/categories.yaml</code>. System rows are
            never modified.
          </p>
          <textarea
            value={importText}
            onChange={(e) => setImportText(e.target.value)}
            placeholder="categories:&#10;  - key: subscriptions&#10;    label: Subscriptions&#10;    subcategories: []"
            rows={10}
            className="w-full px-3 py-2 text-xs font-mono border border-[var(--color-input-border)] bg-[var(--color-input-bg)] text-[var(--color-text)] rounded-lg"
          />
          <label className="flex items-center gap-2 mt-3 text-sm text-[var(--color-text)]">
            <input
              type="checkbox"
              checked={importReplace}
              onChange={(e) => setImportReplace(e.target.checked)}
            />
            Replace mode — remove custom entries not present in the YAML
          </label>
          <div className="mt-4 flex justify-end gap-2">
            <button
              onClick={() => setImportOpen(false)}
              className="px-4 py-2 text-sm text-[var(--color-text-secondary)] hover:text-[var(--color-text)]"
            >
              Cancel
            </button>
            <button
              onClick={() => void handleImport()}
              disabled={importBusy || !importText.trim()}
              className="px-4 py-2 text-sm font-medium bg-[var(--color-primary)] text-white rounded-lg hover:bg-[var(--color-primary-hover)] disabled:opacity-50"
            >
              {importBusy ? 'Importing…' : 'Import'}
            </button>
          </div>
        </Modal>
      )}
    </div>
  );
}

// ── Presentational helpers ─────────────────────────────────────────────

function CategoryRowGroup({ children }: { children: React.ReactNode }) {
  return <>{children}</>;
}

function TypeBadge({ isSystem }: { isSystem: boolean }) {
  return (
    <span
      className={`text-xs px-2 py-0.5 rounded-full ${
        isSystem
          ? 'bg-[var(--color-surface)] text-[var(--color-text-secondary)]'
          : 'bg-[var(--color-primary-subtle)] text-[var(--color-primary)]'
      }`}
    >
      {isSystem ? 'System' : 'Custom'}
    </span>
  );
}

function InlineActions({ onSave, onCancel }: { onSave: () => void; onCancel: () => void }) {
  return (
    <div className="flex gap-2 justify-end">
      <button onClick={onSave} className="text-xs text-[var(--color-primary)] hover:underline">
        Save
      </button>
      <button
        onClick={onCancel}
        className="text-xs text-[var(--color-text-secondary)] hover:underline"
      >
        Cancel
      </button>
    </div>
  );
}

function RowMenu({
  id,
  open,
  setOpen,
  menuRef,
  canDelete,
  canMove,
  onEdit,
  onDelete,
  onMove,
}: {
  id: string;
  open: boolean;
  setOpen: (k: string | null) => void;
  menuRef: React.MutableRefObject<HTMLDivElement | null>;
  canDelete: boolean;
  canMove?: boolean;
  onEdit: () => void;
  onDelete: () => void;
  onMove?: () => void;
}) {
  return (
    <div className="relative inline-block" ref={open ? menuRef : undefined}>
      <button
        onClick={() => setOpen(open ? null : id)}
        className="w-7 h-7 rounded hover:bg-[var(--color-border)] text-[var(--color-text-secondary)]"
        aria-haspopup="menu"
        aria-expanded={open}
        aria-label="Row actions"
      >
        ⋯
      </button>
      {open && (
        <div
          role="menu"
          className="absolute right-0 mt-1 w-36 py-1 bg-[var(--color-card)] border border-[var(--color-border)] rounded-lg shadow-lg z-20"
        >
          <button
            onClick={onEdit}
            className="block w-full text-left px-3 py-1.5 text-xs text-[var(--color-text)] hover:bg-[var(--color-surface)]"
          >
            Edit label
          </button>
          {canMove && onMove && (
            <button
              onClick={onMove}
              className="block w-full text-left px-3 py-1.5 text-xs text-[var(--color-text)] hover:bg-[var(--color-surface)]"
            >
              Move to…
            </button>
          )}
          {canDelete && (
            <button
              onClick={onDelete}
              className="block w-full text-left px-3 py-1.5 text-xs text-[var(--color-error)] hover:bg-[var(--color-surface)]"
            >
              Delete
            </button>
          )}
        </div>
      )}
    </div>
  );
}

function Modal({
  title,
  onClose,
  children,
}: {
  title: string;
  onClose: () => void;
  children: React.ReactNode;
}) {
  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/40"
      onClick={onClose}
    >
      <div
        className="bg-[var(--color-card)] border border-[var(--color-border)] rounded-xl p-5 w-full max-w-lg mx-4"
        onClick={(e) => e.stopPropagation()}
      >
        <h4 className="text-base font-medium text-[var(--color-text)] mb-3">{title}</h4>
        {children}
      </div>
    </div>
  );
}
