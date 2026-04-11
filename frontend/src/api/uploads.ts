import type { Upload, UploadPreview, ColumnMapping } from '@/types/models';
import { useConfigStore } from '@/stores/configStore';
import { useAuthStore } from '@/stores/authStore';

export function createUploadApi(api: {
  get: <T>(path: string) => Promise<T>;
  post: <T>(path: string, body?: unknown) => Promise<T>;
}) {
  return {
    /**
     * Upload a file via multipart form data.
     * This bypasses the JSON-based useApi because we need FormData + progress tracking.
     */
    uploadFile: async (
      accountId: string,
      file: File,
      onProgress?: (percent: number) => void,
    ): Promise<Upload> => {
      const baseUrl = useConfigStore.getState().apiBaseUrl;
      const token = useAuthStore.getState().accessToken;

      const formData = new FormData();
      formData.append('file', file);
      formData.append('account_id', accountId);

      return new Promise<Upload>((resolve, reject) => {
        const xhr = new XMLHttpRequest();
        xhr.open('POST', `${baseUrl}/api/uploads`);
        if (token) {
          xhr.setRequestHeader('Authorization', `Bearer ${token}`);
        }

        xhr.upload.addEventListener('progress', (e) => {
          if (e.lengthComputable && onProgress) {
            onProgress(Math.round((e.loaded / e.total) * 100));
          }
        });

        xhr.addEventListener('load', () => {
          if (xhr.status >= 200 && xhr.status < 300) {
            resolve(JSON.parse(xhr.responseText) as Upload);
          } else {
            reject(new Error(`Upload failed: HTTP ${xhr.status}`));
          }
        });

        xhr.addEventListener('error', () => reject(new Error('Upload failed: network error')));
        xhr.addEventListener('abort', () => reject(new Error('Upload aborted')));

        xhr.send(formData);
      });
    },

    getPreview: (uploadId: string) => api.get<UploadPreview>(`/api/uploads/${uploadId}/preview`),

    confirmUpload: (uploadId: string, mapping: ColumnMapping) =>
      api.post<Upload>(`/api/uploads/${uploadId}/confirm`, mapping),

    getUploadStatus: (uploadId: string) => api.get<Upload>(`/api/uploads/${uploadId}/status`),
  };
}
