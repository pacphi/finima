export interface SubcategoryEntry {
  key: string;
  label: string;
  is_system: boolean;
}

export interface CategoryEntry {
  key: string;
  label: string;
  is_system: boolean;
  subcategories: SubcategoryEntry[];
}

export interface ImportResult {
  created: number;
  updated: number;
  removed: number;
}

export function createCategoryApi(api: {
  get: <T>(path: string) => Promise<T>;
  post: <T>(path: string, body?: unknown) => Promise<T>;
  put: <T>(path: string, body?: unknown) => Promise<T>;
  del: <T>(path: string) => Promise<T>;
}) {
  return {
    listCategories: () => api.get<CategoryEntry[]>('/api/categories'),

    createCategory: (key: string, label: string, parentKey?: string) =>
      api.post<CategoryEntry>('/api/categories', { key, label, parent_key: parentKey }),

    updateCategory: (key: string, label: string, parentKey?: string) =>
      api.put<CategoryEntry>(`/api/categories/${encodeURIComponent(key)}`, {
        label,
        parent_key: parentKey,
      }),

    deleteCategory: (key: string) => api.del<void>(`/api/categories/${encodeURIComponent(key)}`),

    exportYaml: () => api.get<{ yaml: string }>('/api/categories/export'),

    importYaml: (yaml: string, replace = false) =>
      api.post<ImportResult>('/api/categories/import', { yaml, replace }),
  };
}
