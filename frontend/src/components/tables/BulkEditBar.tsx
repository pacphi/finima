import { useState, useRef, useEffect } from 'react';
import type { CategoryEntry } from '@/api/categories';
import type { CategoryMap } from '@/hooks/useCategories';
import { categoryLabel } from '@/hooks/useCategories';

interface FlatItem {
  category: string;
  subcategory?: string;
  label: string;
  parentLabel?: string;
}

function flattenCategories(categories: CategoryEntry[], categoryMap: CategoryMap): FlatItem[] {
  const items: FlatItem[] = [];
  for (const cat of categories) {
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

interface BulkEditBarProps {
  selectedCount: number;
  categories: CategoryEntry[];
  categoryMap: CategoryMap;
  onBulkCategoryChange: (category: string, subcategory?: string) => void;
  onClearSelection: () => void;
}

export function BulkEditBar({
  selectedCount,
  categories,
  categoryMap,
  onBulkCategoryChange,
  onClearSelection,
}: BulkEditBarProps) {
  const [showCategoryPicker, setShowCategoryPicker] = useState(false);
  const [search, setSearch] = useState('');
  const searchInputRef = useRef<HTMLInputElement>(null);
  const [activeIndex, setActiveIndex] = useState(-1);

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

  // Focus search input when picker opens
  useEffect(() => {
    if (showCategoryPicker && searchInputRef.current) {
      searchInputRef.current.focus();
    }
  }, [showCategoryPicker]);

  if (selectedCount === 0) return null;

  const handleSelect = (item: FlatItem) => {
    onBulkCategoryChange(item.category, item.subcategory);
    setShowCategoryPicker(false);
    setSearch('');
    setActiveIndex(-1);
  };

  const handleSearchKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Escape') {
      setShowCategoryPicker(false);
      setSearch('');
      setActiveIndex(-1);
      return;
    }
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      setActiveIndex((prev) => Math.min(prev + 1, filtered.length - 1));
      return;
    }
    if (e.key === 'ArrowUp') {
      e.preventDefault();
      setActiveIndex((prev) => Math.max(prev - 1, 0));
      return;
    }
    if (e.key === 'Enter' && activeIndex >= 0 && filtered[activeIndex]) {
      handleSelect(filtered[activeIndex]);
    }
  };

  return (
    <div
      className="flex items-center gap-3 px-4 py-2 bg-[var(--color-primary-subtle)] border border-[var(--color-primary-muted)] rounded-lg"
      role="toolbar"
      aria-label={`Bulk actions for ${selectedCount} selected transactions`}
    >
      <span className="text-sm font-medium text-[var(--color-primary)]" aria-live="polite">
        {selectedCount} selected
      </span>

      <div className="relative">
        <button
          onClick={() => setShowCategoryPicker(!showCategoryPicker)}
          aria-expanded={showCategoryPicker}
          aria-haspopup="listbox"
          className="px-3 py-1.5 text-sm bg-[var(--color-card)] border border-[var(--color-primary-muted)] rounded-lg hover:bg-[var(--color-primary-subtle)] text-[var(--color-primary)] font-medium"
        >
          Change Category
        </button>

        {showCategoryPicker && (
          <div
            className="absolute z-50 top-full left-0 mt-1 w-64 bg-[var(--color-dropdown-bg,var(--color-surface))] border border-[var(--color-border)] rounded-lg shadow-lg"
            role="dialog"
            aria-label="Select a category"
          >
            <div className="p-2 border-b border-[var(--color-border)]">
              <label htmlFor="bulk-category-search" className="sr-only">
                Search categories
              </label>
              <input
                ref={searchInputRef}
                id="bulk-category-search"
                type="text"
                value={search}
                onChange={(e) => {
                  setSearch(e.target.value);
                  setActiveIndex(-1);
                }}
                onKeyDown={handleSearchKeyDown}
                placeholder="Search categories..."
                className="w-full px-2 py-1 text-sm border border-[var(--color-input-border)] rounded focus:outline-none focus:ring-1 focus:ring-[var(--color-primary)]"
                role="combobox"
                aria-expanded={true}
                aria-controls="bulk-category-listbox"
                aria-activedescendant={activeIndex >= 0 ? `bulk-cat-${activeIndex}` : undefined}
              />
            </div>
            <ul
              id="bulk-category-listbox"
              role="listbox"
              aria-label="Categories"
              className="max-h-48 overflow-y-auto"
            >
              {filtered.map((item, index) => {
                const isSubcategory = !!item.subcategory;
                return (
                  <li
                    key={`${item.category}-${item.subcategory ?? '_'}`}
                    id={`bulk-cat-${index}`}
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
                    >
                      {item.label}
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
        )}
      </div>

      <button
        onClick={onClearSelection}
        className="text-sm text-[var(--color-text-secondary)] hover:text-[var(--color-text)]"
      >
        Clear selection
      </button>
    </div>
  );
}
