import { useCallback } from 'react';
import { useConfigStore } from '@/stores/configStore';
import { useAuthStore } from '@/stores/authStore';

type HttpMethod = 'GET' | 'POST' | 'PUT' | 'DELETE';

async function request<T>(
  method: HttpMethod,
  baseUrl: string,
  path: string,
  accessToken: string | null,
  refreshToken: string | null,
  setTokens: (access: string, refresh: string) => void,
  logout: () => void,
  body?: unknown,
): Promise<T> {
  const url = `${baseUrl}${path}`;

  const headers: Record<string, string> = {
    'Content-Type': 'application/json',
  };

  if (accessToken) {
    headers['Authorization'] = `Bearer ${accessToken}`;
  }

  const init: RequestInit = { method, headers };
  if (body !== undefined) {
    init.body = JSON.stringify(body);
  }

  let response = await fetch(url, init);

  // On 401, attempt token refresh and retry once
  if (response.status === 401 && refreshToken) {
    try {
      const refreshResponse = await fetch(`${baseUrl}/api/auth/refresh`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ refresh_token: refreshToken }),
      });

      if (refreshResponse.ok) {
        const tokens = (await refreshResponse.json()) as {
          access_token: string;
          refresh_token: string;
        };
        setTokens(tokens.access_token, tokens.refresh_token);

        headers['Authorization'] = `Bearer ${tokens.access_token}`;
        response = await fetch(url, { method, headers, body: init.body });
      } else {
        logout();
        throw new Error('Session expired');
      }
    } catch {
      logout();
      throw new Error('Session expired');
    }
  }

  if (!response.ok) {
    const errorBody = await response.text();
    throw new Error(`HTTP ${response.status}: ${errorBody}`);
  }

  const text = await response.text();
  if (!text) return undefined as T;
  return JSON.parse(text) as T;
}

export function useApi() {
  const apiBaseUrl = useConfigStore((s) => s.apiBaseUrl);
  const accessToken = useAuthStore((s) => s.accessToken);
  const refreshToken = useAuthStore((s) => s.refreshToken);
  const setTokens = useAuthStore((s) => s.setTokens);
  const logout = useAuthStore((s) => s.logout);

  const get = useCallback(
    <T>(path: string) =>
      request<T>('GET', apiBaseUrl, path, accessToken, refreshToken, setTokens, logout),
    [apiBaseUrl, accessToken, refreshToken, setTokens, logout],
  );

  const post = useCallback(
    <T>(path: string, body?: unknown) =>
      request<T>('POST', apiBaseUrl, path, accessToken, refreshToken, setTokens, logout, body),
    [apiBaseUrl, accessToken, refreshToken, setTokens, logout],
  );

  const put = useCallback(
    <T>(path: string, body?: unknown) =>
      request<T>('PUT', apiBaseUrl, path, accessToken, refreshToken, setTokens, logout, body),
    [apiBaseUrl, accessToken, refreshToken, setTokens, logout],
  );

  const del = useCallback(
    <T>(path: string) =>
      request<T>('DELETE', apiBaseUrl, path, accessToken, refreshToken, setTokens, logout),
    [apiBaseUrl, accessToken, refreshToken, setTokens, logout],
  );

  return { get, post, put, del };
}
