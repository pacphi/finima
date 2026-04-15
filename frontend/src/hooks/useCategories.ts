import { useState, useEffect, useMemo, useCallback } from 'react';
import { useApi } from './useApi';
import { createCategoryApi, type CategoryEntry, type SubcategoryEntry } from '@/api/categories';
import { toTitleCase } from '@/utils/format';

/** Map from category or subcategory key to display label. */
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
 * Returns the hierarchical list, a key->label map (including subcategories), and helpers.
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
        // Sort categories alphabetically, and subcategories within each category
        const sorted = [...data]
          .sort((a, b) => a.label.localeCompare(b.label))
          .map((cat) => ({
            ...cat,
            subcategories: cat.subcategories
              ? [...cat.subcategories].sort((a, b) => a.label.localeCompare(b.label))
              : cat.subcategories,
          }));
        cachedCategories = sorted;
        setCategories(sorted);
      })
      .catch(console.error);
  }, [version]); // eslint-disable-line react-hooks/exhaustive-deps

  /** Maps both parent category keys and subcategory keys to their display labels. */
  const categoryMap: CategoryMap = useMemo(() => {
    const map: CategoryMap = {};
    for (const c of categories) {
      map[c.key] = c.label;
      for (const sub of c.subcategories ?? []) {
        map[sub.key] = sub.label;
      }
    }
    return map;
  }, [categories]);

  /** Returns the subcategories for a given parent category key. */
  const subcategoriesFor = useCallback(
    (categoryKey: string): SubcategoryEntry[] => {
      return categories.find((c) => c.key === categoryKey)?.subcategories ?? [];
    },
    [categories],
  );

  return { categories, categoryMap, subcategoriesFor, refresh };
}

/** Convert a snake_case category key to a display label using the map, with fallback. */
export function categoryLabel(key: string | null, map: CategoryMap): string {
  if (!key) return 'Uncategorized';
  if (map[key]) return map[key];
  // Fallback: convert snake_case to Title Case
  return toTitleCase(key);
}
