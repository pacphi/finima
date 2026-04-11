import { useConfigStore } from '@/stores/configStore';

function getBaseUrl(): string {
  return useConfigStore.getState().apiBaseUrl;
}

async function jsonPost<T>(path: string, body: unknown): Promise<T> {
  const response = await fetch(`${getBaseUrl()}${path}`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  });

  if (!response.ok) {
    const errorBody = await response.text();
    throw new Error(`HTTP ${response.status}: ${errorBody}`);
  }

  const text = await response.text();
  if (!text) return undefined as T;
  return JSON.parse(text) as T;
}

export interface MagicLinkResponse {
  message: string;
}

export interface VerifyResponse {
  access_token: string;
  refresh_token: string;
  user: {
    id: string;
    email: string;
    display_name: string;
  };
  is_new_user: boolean;
}

export interface RefreshResponse {
  access_token: string;
  refresh_token: string;
}

export function requestMagicLink(email: string): Promise<MagicLinkResponse> {
  return jsonPost<MagicLinkResponse>('/api/auth/magic-link', { email });
}

export function verifyToken(token: string, email: string): Promise<VerifyResponse> {
  return jsonPost<VerifyResponse>('/api/auth/verify', { token, email });
}

export function refreshSession(refreshToken: string): Promise<RefreshResponse> {
  return jsonPost<RefreshResponse>('/api/auth/refresh', {
    refresh_token: refreshToken,
  });
}

export async function logout(): Promise<void> {
  const response = await fetch(`${getBaseUrl()}/api/auth/session`, {
    method: 'DELETE',
  });
  if (!response.ok) {
    throw new Error(`Logout failed: HTTP ${response.status}`);
  }
}
