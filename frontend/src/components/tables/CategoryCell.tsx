import { useState, useRef, useEffect } from 'react';

interface CategoryCellProps {
  value: string | null;
  confidence: number | null;
  userOverridden: boolean;
  allCategories: string[];
  onChange: (category: string) => void;
}

export function CategoryCell({
  value,
  confidence,
  userOverridden,
  allCategories,
  onChange,
}: CategoryCellProps) {
  const [editing, setEditing] = useState(false);
  const [search, setSearch] = useState('');
  const [activeIndex, setActiveIndex] = useState(-1);
  const inputRef = useRef<HTMLInputElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  const isLowConfidence = confidence !== null && confidence < 0.7 && !userOverridden;

  const filtered = allCategories.filter((c) => c.toLowerCase().includes(search.toLowerCase()));

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

  const handleSelect = (category: string) => {
    onChange(category);
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
        aria-label={`Category: ${value ?? 'Uncategorized'}${confidenceText}. Click to change.`}
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
        <span className="group-hover:text-[var(--color-primary)] transition-colors">
          {value ?? 'Uncategorized'}
        </span>
        {isLowConfidence && <span className="sr-only"> (low confidence)</span>}
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
        className="w-full px-2 py-1 text-sm border border-[var(--color-primary)] rounded focus:outline-none focus:ring-1 focus:ring-[var(--color-primary)]"
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
        className="absolute z-50 top-full left-0 right-0 mt-1 max-h-48 overflow-y-auto bg-[var(--color-card)] border border-[var(--color-border)] rounded-lg shadow-lg"
      >
        {filtered.map((cat, index) => (
          <li
            key={cat}
            id={`cat-option-${index}`}
            role="option"
            aria-selected={index === activeIndex}
          >
            <button
              onClick={() => handleSelect(cat)}
              className={`block w-full text-left px-3 py-1.5 text-sm hover:bg-[var(--color-primary-subtle)] hover:text-[var(--color-primary)] ${
                index === activeIndex
                  ? 'bg-[var(--color-primary-subtle)] text-[var(--color-primary)]'
                  : ''
              }`}
              tabIndex={-1}
            >
              {cat}
            </button>
          </li>
        ))}
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
