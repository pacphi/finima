import { useState, useMemo, useCallback } from 'react';
import {
  useReactTable,
  getCoreRowModel,
  flexRender,
  type ColumnDef,
  type SortingState,
} from '@tanstack/react-table';
import { CategoryCell } from './CategoryCell';
import { BulkEditBar } from './BulkEditBar';
import type { Transaction, TransactionFilters, Account } from '@/types/models';

// ── Helpers ──────────────────────────────────────────────────────────

function formatAmount(amount: number): { text: string; className: string; srText: string } {
  const abs = Math.abs(amount).toLocaleString('en-US', {
    style: 'currency',
    currency: 'USD',
  });
  if (amount < 0) {
    return {
      text: `-${abs.replace('-', '')}`,
      className: 'text-red-600 font-medium',
      srText: `${abs.replace('-', '')} (expense)`,
    };
  }
  return {
    text: `+${abs}`,
    className: 'text-green-600 font-medium',
    srText: `${abs} (income)`,
  };
}

function formatDate(dateStr: string): string {
  const d = new Date(dateStr);
  return d.toLocaleDateString('en-US', { month: 'short', day: '2-digit', year: 'numeric' });
}

// ── Props ────────────────────────────────────────────────────────────

interface TransactionTableProps {
  transactions: Transaction[];
  total: number;
  page: number;
  perPage: number;
  sorting: SortingState;
  filters: TransactionFilters;
  accounts: Account[];
  allCategories: string[];
  lockedAccountId?: string;
  onSortingChange: (sorting: SortingState) => void;
  onPageChange: (page: number) => void;
  onFiltersChange: (filters: TransactionFilters) => void;
  onCategoryChange: (transactionId: string, category: string) => void;
  onBulkCategoryChange: (transactionIds: string[], category: string) => void;
  onExportCsv: () => void;
}

export function TransactionTable({
  transactions,
  total,
  page,
  perPage,
  sorting,
  filters,
  accounts,
  allCategories,
  lockedAccountId,
  onSortingChange,
  onPageChange,
  onFiltersChange,
  onCategoryChange,
  onBulkCategoryChange,
  onExportCsv,
}: TransactionTableProps) {
  'use no memo'; // TanStack Table's useReactTable returns mutable objects incompatible with React Compiler
  const [rowSelection, setRowSelection] = useState<Record<string, boolean>>({});
  const [filtersExpanded, setFiltersExpanded] = useState(false);

  const selectedIds = useMemo(
    () =>
      Object.entries(rowSelection)
        .filter(([, selected]) => selected)
        .map(([idx]) => transactions[Number(idx)]?.id)
        .filter((id): id is string => id !== undefined),
    [rowSelection, transactions],
  );

  const accountLookup = useMemo(() => {
    const map = new Map<string, string>();
    for (const a of accounts) map.set(a.id, a.name);
    return map;
  }, [accounts]);

  const columns = useMemo<ColumnDef<Transaction>[]>(() => {
    const cols: ColumnDef<Transaction>[] = [
      {
        id: 'select',
        header: ({ table }) => (
          <label className="inline-flex items-center">
            <input
              type="checkbox"
              checked={table.getIsAllPageRowsSelected()}
              onChange={table.getToggleAllPageRowsSelectedHandler()}
              className="rounded border-[var(--color-input-border)]"
              aria-label="Select all transactions on this page"
            />
          </label>
        ),
        cell: ({ row }) => (
          <label className="inline-flex items-center">
            <input
              type="checkbox"
              checked={row.getIsSelected()}
              onChange={row.getToggleSelectedHandler()}
              className="rounded border-[var(--color-input-border)]"
              aria-label={`Select transaction: ${row.original.description}`}
            />
          </label>
        ),
        size: 40,
        enableSorting: false,
      },
      {
        accessorKey: 'date',
        header: 'Date',
        cell: ({ getValue }) => formatDate(getValue<string>()),
        size: 120,
      },
      {
        accessorKey: 'description',
        header: 'Description',
        size: 250,
      },
      {
        accessorKey: 'category',
        header: 'Category',
        cell: ({ row }) => (
          <CategoryCell
            value={row.original.category}
            confidence={row.original.llm_confidence}
            userOverridden={row.original.user_overridden}
            allCategories={allCategories}
            onChange={(cat) => onCategoryChange(row.original.id, cat)}
          />
        ),
        size: 180,
        // Hide on mobile via meta
        meta: { hideOnMobile: true },
      },
      {
        accessorKey: 'amount',
        header: 'Amount',
        cell: ({ getValue }) => {
          const { text, className, srText } = formatAmount(getValue<number>());
          return (
            <span className={className}>
              <span aria-hidden="true">{text}</span>
              <span className="sr-only">{srText}</span>
            </span>
          );
        },
        size: 120,
      },
    ];

    if (!lockedAccountId) {
      cols.push({
        accessorKey: 'account_id',
        header: 'Account',
        cell: ({ getValue }) => accountLookup.get(getValue<string>()) ?? 'Unknown',
        size: 140,
        // Hide on mobile
        meta: { hideOnMobile: true },
      });
    }

    return cols;
  }, [allCategories, onCategoryChange, lockedAccountId, accountLookup]);

  // eslint-disable-next-line react-hooks/incompatible-library -- opted out via "use no memo"; tracked by tanstack/table#5567
  const table = useReactTable({
    data: transactions,
    columns,
    state: { sorting, rowSelection },
    onSortingChange: (updater) => {
      const newSorting = typeof updater === 'function' ? updater(sorting) : updater;
      onSortingChange(newSorting);
    },
    onRowSelectionChange: setRowSelection,
    getCoreRowModel: getCoreRowModel(),
    manualSorting: true,
    manualPagination: true,
    pageCount: Math.ceil(total / perPage),
    enableRowSelection: true,
    getRowId: (row) => row.id,
  });

  const totalPages = Math.ceil(total / perPage);
  const startItem = (page - 1) * perPage + 1;
  const endItem = Math.min(page * perPage, total);

  const handleFilterChange = useCallback(
    (key: keyof TransactionFilters, value: string | number | undefined) => {
      onFiltersChange({ ...filters, [key]: value || undefined });
    },
    [filters, onFiltersChange],
  );

  return (
    <div className="space-y-4">
      {/* Filter bar: collapsible on mobile */}
      <div>
        <button
          onClick={() => setFiltersExpanded(!filtersExpanded)}
          aria-expanded={filtersExpanded}
          aria-controls="transaction-filters"
          className="md:hidden mb-2 px-3 py-1.5 text-sm border border-[var(--color-border)] text-[var(--color-text-secondary)] rounded-lg w-full text-left"
        >
          {filtersExpanded ? 'Hide Filters' : 'Show Filters'}
        </button>
        <div
          id="transaction-filters"
          className={`${filtersExpanded ? 'block' : 'hidden'} md:block`}
        >
          <div className="flex flex-wrap gap-2 items-end">
            <div>
              <label className="block text-xs font-medium text-[var(--color-text-secondary)] mb-1">
                From
              </label>
              <input
                type="date"
                value={filters.date_from ?? ''}
                onChange={(e) => handleFilterChange('date_from', e.target.value)}
                className="px-2 py-1.5 text-sm border border-[var(--color-input-border)] bg-[var(--color-input-bg)] text-[var(--color-text)] rounded-lg focus:border-[var(--color-primary)] focus:ring-1 focus:ring-[var(--color-primary)]"
              />
            </div>
            <div>
              <label className="block text-xs font-medium text-[var(--color-text-secondary)] mb-1">
                To
              </label>
              <input
                type="date"
                value={filters.date_to ?? ''}
                onChange={(e) => handleFilterChange('date_to', e.target.value)}
                className="px-2 py-1.5 text-sm border border-[var(--color-input-border)] bg-[var(--color-input-bg)] text-[var(--color-text)] rounded-lg focus:border-[var(--color-primary)] focus:ring-1 focus:ring-[var(--color-primary)]"
              />
            </div>
            {!lockedAccountId && (
              <div>
                <label className="block text-xs font-medium text-[var(--color-text-secondary)] mb-1">
                  Account
                </label>
                <select
                  value={filters.account_id ?? ''}
                  onChange={(e) => handleFilterChange('account_id', e.target.value)}
                  className="px-2 py-1.5 text-sm border border-[var(--color-input-border)] bg-[var(--color-input-bg)] text-[var(--color-text)] rounded-lg focus:border-[var(--color-primary)] focus:ring-1 focus:ring-[var(--color-primary)]"
                >
                  <option value="">All Accounts</option>
                  {accounts.map((a) => (
                    <option key={a.id} value={a.id}>
                      {a.name}
                    </option>
                  ))}
                </select>
              </div>
            )}
            <div>
              <label className="block text-xs font-medium text-[var(--color-text-secondary)] mb-1">
                Category
              </label>
              <select
                value={filters.category ?? ''}
                onChange={(e) => handleFilterChange('category', e.target.value)}
                className="px-2 py-1.5 text-sm border border-[var(--color-input-border)] bg-[var(--color-input-bg)] text-[var(--color-text)] rounded-lg focus:border-[var(--color-primary)] focus:ring-1 focus:ring-[var(--color-primary)]"
              >
                <option value="">All Categories</option>
                {allCategories.map((c) => (
                  <option key={c} value={c}>
                    {c}
                  </option>
                ))}
              </select>
            </div>
            <div>
              <label className="block text-xs font-medium text-[var(--color-text-secondary)] mb-1">
                Search
              </label>
              <input
                type="text"
                value={filters.search ?? ''}
                onChange={(e) => handleFilterChange('search', e.target.value)}
                placeholder="Search..."
                className="px-2 py-1.5 text-sm border border-[var(--color-input-border)] bg-[var(--color-input-bg)] text-[var(--color-text)] rounded-lg focus:border-[var(--color-primary)] focus:ring-1 focus:ring-[var(--color-primary)]"
              />
            </div>
            <div className="hidden md:block">
              <label className="block text-xs font-medium text-[var(--color-text-secondary)] mb-1">
                Min Amount
              </label>
              <input
                type="number"
                value={filters.amount_min ?? ''}
                onChange={(e) =>
                  handleFilterChange(
                    'amount_min',
                    e.target.value ? Number(e.target.value) : undefined,
                  )
                }
                placeholder="Min"
                className="w-24 px-2 py-1.5 text-sm border border-[var(--color-input-border)] bg-[var(--color-input-bg)] text-[var(--color-text)] rounded-lg focus:border-[var(--color-primary)] focus:ring-1 focus:ring-[var(--color-primary)]"
              />
            </div>
            <div className="hidden md:block">
              <label className="block text-xs font-medium text-[var(--color-text-secondary)] mb-1">
                Max Amount
              </label>
              <input
                type="number"
                value={filters.amount_max ?? ''}
                onChange={(e) =>
                  handleFilterChange(
                    'amount_max',
                    e.target.value ? Number(e.target.value) : undefined,
                  )
                }
                placeholder="Max"
                className="w-24 px-2 py-1.5 text-sm border border-[var(--color-input-border)] bg-[var(--color-input-bg)] text-[var(--color-text)] rounded-lg focus:border-[var(--color-primary)] focus:ring-1 focus:ring-[var(--color-primary)]"
              />
            </div>
            <button
              onClick={() =>
                onFiltersChange(lockedAccountId ? { account_id: lockedAccountId } : {})
              }
              className="px-3 py-1.5 text-sm bg-[var(--color-surface)] text-[var(--color-text-secondary)] rounded-lg hover:bg-[var(--color-border)] transition-colors"
            >
              Clear
            </button>
          </div>
        </div>
      </div>

      {/* Action bar */}
      <div className="flex items-center justify-between">
        <div className="text-sm text-[var(--color-text-secondary)]" aria-live="polite">
          Showing {total > 0 ? startItem : 0}-{endItem} of {total} transactions
        </div>
        <div className="flex items-center gap-2">
          <BulkEditBar
            selectedCount={selectedIds.length}
            allCategories={allCategories}
            onBulkCategoryChange={(cat) => {
              onBulkCategoryChange(selectedIds, cat);
              setRowSelection({});
            }}
            onClearSelection={() => setRowSelection({})}
          />
          <button
            onClick={onExportCsv}
            className="px-3 py-1.5 text-sm bg-[var(--color-surface)] text-[var(--color-text-secondary)] rounded-lg hover:bg-[var(--color-border)] transition-colors"
          >
            Export CSV
          </button>
        </div>
      </div>

      {/* Table with horizontal scroll on mobile */}
      <div className="border border-[var(--color-border)] rounded-lg overflow-x-auto">
        <table className="w-full text-sm" aria-label="Transactions">
          <thead className="bg-[var(--color-surface)] border-b border-[var(--color-border)]">
            {table.getHeaderGroups().map((headerGroup) => (
              <tr key={headerGroup.id}>
                {headerGroup.headers.map((header) => {
                  const hideOnMobile = (
                    header.column.columnDef.meta as Record<string, boolean> | undefined
                  )?.hideOnMobile;
                  const sortDir = header.column.getIsSorted();
                  const ariaSortValue =
                    sortDir === 'asc'
                      ? ('ascending' as const)
                      : sortDir === 'desc'
                        ? ('descending' as const)
                        : header.column.getCanSort()
                          ? ('none' as const)
                          : undefined;
                  return (
                    <th
                      key={header.id}
                      scope="col"
                      aria-sort={ariaSortValue}
                      className={`text-left px-3 py-2.5 text-xs font-medium text-[var(--color-text-secondary)] uppercase tracking-wider ${
                        header.column.getCanSort()
                          ? 'cursor-pointer select-none hover:text-[var(--color-text)]'
                          : ''
                      } ${hideOnMobile ? 'hidden md:table-cell' : ''}`}
                      style={{ width: header.getSize() }}
                      onClick={header.column.getToggleSortingHandler()}
                    >
                      <div className="flex items-center gap-1">
                        {header.isPlaceholder
                          ? null
                          : flexRender(header.column.columnDef.header, header.getContext())}
                        {header.column.getIsSorted() === 'asc' && (
                          <span aria-hidden="true"> {'\u2191'}</span>
                        )}
                        {header.column.getIsSorted() === 'desc' && (
                          <span aria-hidden="true"> {'\u2193'}</span>
                        )}
                      </div>
                    </th>
                  );
                })}
              </tr>
            ))}
          </thead>
          <tbody className="divide-y divide-[var(--color-border)]">
            {table.getRowModel().rows.length === 0 ? (
              <tr>
                <td
                  colSpan={columns.length}
                  className="px-3 py-8 text-center text-[var(--color-text-secondary)]"
                >
                  No transactions found
                </td>
              </tr>
            ) : (
              table.getRowModel().rows.map((row) => (
                <tr key={row.id} className="hover:bg-[var(--color-surface)] transition-colors">
                  {row.getVisibleCells().map((cell) => {
                    const hideOnMobile = (
                      cell.column.columnDef.meta as Record<string, boolean> | undefined
                    )?.hideOnMobile;
                    return (
                      <td
                        key={cell.id}
                        className={`px-3 py-2.5 ${hideOnMobile ? 'hidden md:table-cell' : ''}`}
                      >
                        {flexRender(cell.column.columnDef.cell, cell.getContext())}
                      </td>
                    );
                  })}
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>

      {/* Pagination */}
      {totalPages > 1 && (
        <div className="flex flex-col sm:flex-row items-center justify-between gap-2">
          <div className="text-sm text-[var(--color-text-secondary)]">
            Page {page} of {totalPages}
          </div>
          <div className="flex items-center gap-1">
            <button
              onClick={() => onPageChange(page - 1)}
              disabled={page <= 1}
              className="px-3 py-1.5 text-sm border border-[var(--color-border)] rounded-lg disabled:opacity-50 hover:bg-[var(--color-surface)] text-[var(--color-text)]"
            >
              Previous
            </button>
            {Array.from({ length: Math.min(totalPages, 7) }, (_, i) => {
              let pageNum: number;
              if (totalPages <= 7) {
                pageNum = i + 1;
              } else if (page <= 4) {
                pageNum = i + 1;
              } else if (page >= totalPages - 3) {
                pageNum = totalPages - 6 + i;
              } else {
                pageNum = page - 3 + i;
              }
              return (
                <button
                  key={pageNum}
                  onClick={() => onPageChange(pageNum)}
                  className={`px-3 py-1.5 text-sm rounded-lg hidden sm:inline-block ${
                    pageNum === page
                      ? 'bg-[var(--color-primary)] text-white'
                      : 'border border-[var(--color-border)] hover:bg-[var(--color-surface)] text-[var(--color-text)]'
                  }`}
                >
                  {pageNum}
                </button>
              );
            })}
            <button
              onClick={() => onPageChange(page + 1)}
              disabled={page >= totalPages}
              className="px-3 py-1.5 text-sm border border-[var(--color-border)] rounded-lg disabled:opacity-50 hover:bg-[var(--color-surface)] text-[var(--color-text)]"
            >
              Next
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
