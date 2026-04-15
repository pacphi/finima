import { useState, useRef, useEffect } from 'react';
import type { CategoryEntry } from '@/api/categories';
import type { CategoryMap } from '@/hooks/useCategories';
import { categoryLabel } from '@/hooks/useCategories';

interface CategoryCellProps {
  value: string | null;
  subcategory?: string | null;
  confidence: number | null;
  userOverridden: boolean;
  categories: CategoryEntry[];
  categoryMap: CategoryMap;
  /** Called immediately on selection (old behaviour, used by non-transaction contexts). */
  onChange?: (category: string, subcategory?: string) => void;
  /**
   * When provided, selecting a category calls this instead of onChange.
   * The parent is responsible for showing Apply/Cancel controls and calling
   * the real save when the user confirms.
   */
  onPendingChange?: (category: string, subcategory?: string) => void;
  /** If set, the cell displays this staged value instead of the committed one. */
  pendingCategory?: string | null;
  pendingSubcategory?: string | null;
}

/** Build a flat list of selectable items from the hierarchy for search/filter. */
interface FlatItem {
  category: string;
  subcategory?: string;
  label: string;
  parentLabel?: string;
}

function flattenCategories(categories: CategoryEntry[], categoryMap: CategoryMap): FlatItem[] {
  const items: FlatItem[] = [];
  for (const cat of categories) {
    // Parent entry (selecting it means "no subcategory")
    items.push({
      category: cat.key,
      label: categoryLabel(cat.key, categoryMap),
    });
    for (const sub of cat.subcategories ?? []) {
      items.push({
        category: cat.key,
        subcategory: sub.key,
        label: categoryLabel(sub.key, categoryMap),
        parentLabel: categoryLabel(cat.key, categoryMap),
      });
    }
  }
  return items;
}

export function CategoryCell({
  value,
  subcategory: _subcategory,
  confidence,
  userOverridden,
  categories,
  categoryMap,
  onChange,
  onPendingChange,
  pendingCategory,
  pendingSubcategory: _pendingSubcategory,
}: CategoryCellProps) {
  const [editing, setEditing] = useState(false);
  const [search, setSearch] = useState('');
  const [activeIndex, setActiveIndex] = useState(-1);
  const inputRef = useRef<HTMLInputElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  // When a pending change exists, show the pending value; low-confidence warning
  // is suppressed once the user has staged a change.
  const hasPending = pendingCategory != null;
  const displayValue = hasPending ? pendingCategory : value;
  const isLowConfidence = confidence !== null && confidence < 0.7 && !userOverridden && !hasPending;

  const displayLabel = categoryLabel(displayValue, categoryMap);

  const allItems = flattenCategories(categories, categoryMap);

  const filtered = allItems.filter((item) => {
    const q = search.toLowerCase();
    return (
      item.label.toLowerCase().includes(q) ||
      item.category.toLowerCase().includes(q) ||
      (item.parentLabel?.toLowerCase().includes(q) ?? false) ||
      (item.subcategory?.toLowerCase().includes(q) ?? false)
    );
  });

  useEffect(() => {
    if (editing && inputRef.current) {
      inputRef.current.focus();
    }
  }, [editing]);

  useEffect(() => {
    function handleClickOutside(e: MouseEvent) {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        setEditing(false);
      }
    }
    if (editing) {
      document.addEventListener('mousedown', handleClickOutside);
    }
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, [editing]);

  const handleSelect = (item: FlatItem) => {
    if (onPendingChange) {
      onPendingChange(item.category, item.subcategory);
    } else {
      onChange?.(item.category, item.subcategory);
    }
    setEditing(false);
    setSearch('');
    setActiveIndex(-1);
  };

  const confidenceText = isLowConfidence ? ' (low confidence)' : '';

  if (!editing) {
    return (
      <button
        onClick={() => setEditing(true)}
        className="flex items-center gap-1 text-left w-full group"
        aria-label={`Category: ${displayLabel}${confidenceText}${hasPending ? ' (unsaved)' : ''}. Click to change.`}
      >
        {isLowConfidence && (
          <span
            className="text-amber-500 text-xs"
            title="Low confidence categorization"
            aria-hidden="true"
          >
            ⚠️
          </span>
        )}
        <span
          className={`group-hover:text-[var(--color-primary)] transition-colors ${
            hasPending ? 'italic text-[var(--color-primary)]' : ''
          }`}
        >
          {displayLabel}
        </span>
        {hasPending && (
          <span
            className="text-[var(--color-primary)] text-xs"
            title="Unsaved change"
            aria-hidden="true"
          >
            *
          </span>
        )}
        {isLowConfidence && <span className="sr-only"> (low confidence)</span>}
        {hasPending && <span className="sr-only"> (unsaved)</span>}
      </button>
    );
  }

  return (
    <div ref={containerRef} className="relative">
      <label htmlFor="category-search-input" className="sr-only">
        Search and select a category
      </label>
      <input
        ref={inputRef}
        id="category-search-input"
        type="text"
        value={search}
        onChange={(e) => {
          setSearch(e.target.value);
          setActiveIndex(-1);
        }}
        placeholder="Search categories..."
        className="w-full px-2 py-1 text-sm border border-[var(--color-primary)] rounded bg-[var(--color-dropdown-bg,var(--color-surface))] text-[var(--color-text)] focus:outline-none focus:ring-1 focus:ring-[var(--color-primary)]"
        role="combobox"
        aria-expanded={true}
        aria-controls="category-listbox"
        aria-activedescendant={activeIndex >= 0 ? `cat-option-${activeIndex}` : undefined}
        onKeyDown={(e) => {
          if (e.key === 'Escape') {
            setEditing(false);
            setSearch('');
            setActiveIndex(-1);
          }
          if (e.key === 'ArrowDown') {
            e.preventDefault();
            setActiveIndex((prev) => Math.min(prev + 1, filtered.length - 1));
          }
          if (e.key === 'ArrowUp') {
            e.preventDefault();
            setActiveIndex((prev) => Math.max(prev - 1, 0));
          }
          if (e.key === 'Enter') {
            if (activeIndex >= 0 && filtered[activeIndex]) {
              handleSelect(filtered[activeIndex]);
            } else if (filtered.length > 0 && filtered[0]) {
              handleSelect(filtered[0]);
            }
          }
        }}
      />
      <ul
        id="category-listbox"
        role="listbox"
        aria-label="Available categories"
        className="absolute z-50 top-full left-0 right-0 mt-1 max-h-48 overflow-y-auto bg-[var(--color-dropdown-bg,var(--color-surface))] border border-[var(--color-border)] rounded-lg shadow-lg"
      >
        {filtered.map((item, index) => {
          const isSubcategory = !!item.subcategory;
          const itemLabel = isSubcategory ? `${item.parentLabel} > ${item.label}` : item.label;
          return (
            <li
              key={`${item.category}-${item.subcategory ?? '_'}`}
              id={`cat-option-${index}`}
              role="option"
              aria-selected={index === activeIndex}
            >
              <button
                onClick={() => handleSelect(item)}
                className={`block w-full text-left px-3 py-1.5 text-sm hover:bg-[var(--color-primary-subtle)] hover:text-[var(--color-primary)] ${
                  isSubcategory ? 'pl-6' : 'font-medium'
                } ${
                  index === activeIndex
                    ? 'bg-[var(--color-primary-subtle)] text-[var(--color-primary)]'
                    : ''
                }`}
                tabIndex={-1}
              >
                {isSubcategory ? item.label : itemLabel}
              </button>
            </li>
          );
        })}
        {filtered.length === 0 && (
          <li
            className="px-3 py-2 text-sm text-[var(--color-text-secondary)]"
            role="option"
            aria-selected={false}
            aria-disabled={true}
          >
            No matching categories
          </li>
        )}
      </ul>
    </div>
  );
}
