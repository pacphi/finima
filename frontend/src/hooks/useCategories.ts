import { useState, useEffect, useMemo, useCallback } from 'react';
import { useApi } from './useApi';
import { createCategoryApi, type CategoryEntry } from '@/api/categories';

/** Map from category key (e.g. "food_dining") to display label (e.g. "Food & Dining"). */
export type CategoryMap = Record<string, string>;

let cachedCategories: CategoryEntry[] | null = null;
let cacheVersion = 0;

/** Invalidate the category cache so the next useCategories call refetches. */
export function invalidateCategoryCache() {
  cachedCategories = null;
  cacheVersion++;
}

/**
 * Loads categories from the API (cached after first load).
 * Returns the raw list and a key->label map for display.
 */
export function useCategories() {
  const api = useApi();
  const categoryApi = createCategoryApi(api);
  const [categories, setCategories] = useState<CategoryEntry[]>(cachedCategories ?? []);
  const [version, setVersion] = useState(cacheVersion);

  const refresh = useCallback(() => {
    cachedCategories = null;
    cacheVersion++;
    setVersion(cacheVersion);
  }, []);

  useEffect(() => {
    if (cachedCategories && version === cacheVersion) {
      setCategories(cachedCategories);
      return;
    }
    categoryApi
      .listCategories()
      .then((data) => {
        cachedCategories = data;
        setCategories(data);
      })
      .catch(console.error);
  }, [version]); // eslint-disable-line react-hooks/exhaustive-deps

  const categoryMap: CategoryMap = useMemo(() => {
    const map: CategoryMap = {};
    for (const c of categories) {
      map[c.key] = c.label;
    }
    return map;
  }, [categories]);

  return { categories, categoryMap, refresh };
}

/** Convert a snake_case category key to a display label using the map, with fallback. */
export function categoryLabel(key: string | null, map: CategoryMap): string {
  if (!key) return 'Uncategorized';
  if (map[key]) return map[key];
  // Fallback: convert snake_case to Title Case
  return key
    .split('_')
    .map((w) => w.charAt(0).toUpperCase() + w.slice(1))
    .join(' ');
}
