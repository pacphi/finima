import { type Page, type APIRequestContext, expect } from '@playwright/test';

const API_BASE = process.env.E2E_API_BASE_URL || 'http://localhost:3000';

/**
 * Request a magic link for the given email via the backend API.
 * In dev mode the LoggingEmailSender prints the magic link to stdout,
 * but for E2E we call the verify endpoint directly with the token
 * obtained from the dev-mode response or a test-only endpoint.
 */
export async function requestMagicLink(
  request: APIRequestContext,
  email: string,
): Promise<void> {
  const response = await request.post(`${API_BASE}/api/auth/magic-link`, {
    data: { email },
  });
  expect(response.ok()).toBeTruthy();
}

/**
 * Verify a magic link token and return the auth result.
 * In test/dev mode the backend exposes a helper endpoint that returns
 * the most recent token for a given email, avoiding the need to parse
 * server stdout. If that endpoint is unavailable, we fall back to the
 * standard verify endpoint with a well-known test token.
 */
export async function verifyMagicLink(
  request: APIRequestContext,
  email: string,
): Promise<{
  access_token: string;
  refresh_token: string;
  user: { id: string; email: string; display_name: string };
  is_new_user: boolean;
}> {
  // Try the dev-mode test endpoint first -- it returns the latest token
  const tokenResponse = await request.get(
    `${API_BASE}/api/auth/dev/latest-token?email=${encodeURIComponent(email)}`,
  );

  let token: string;
  if (tokenResponse.ok()) {
    const body = await tokenResponse.json();
    token = body.token;
  } else {
    // Fallback: in APP_ENV=test the backend accepts the literal string
    // "test-token" for any email that had a magic link requested.
    token = 'test-token';
  }

  const verifyResponse = await request.post(`${API_BASE}/api/auth/verify`, {
    data: { token, email },
  });
  expect(verifyResponse.ok()).toBeTruthy();
  return verifyResponse.json();
}

/**
 * Perform a full sign-in flow via the API and inject the session into
 * the browser page so subsequent navigations are authenticated.
 */
export async function signIn(
  page: Page,
  request: APIRequestContext,
  email: string,
): Promise<void> {
  await requestMagicLink(request, email);
  const authResult = await verifyMagicLink(request, email);

  // Inject the auth state into sessionStorage so the Zustand authStore
  // picks it up on the next page load.
  const storageValue = JSON.stringify({
    user: {
      id: authResult.user.id,
      email: authResult.user.email,
      displayName: authResult.user.display_name,
    },
    accessToken: authResult.access_token,
    refreshToken: authResult.refresh_token,
  });

  await page.addInitScript((value: string) => {
    sessionStorage.setItem('finima-auth', value);
  }, storageValue);

  // Navigate to trigger the app to load with the stored session.
  await page.goto('/dashboard');
}

/**
 * Sign out the current user by clicking the Logout button in the header.
 */
export async function signOut(page: Page): Promise<void> {
  await page.getByRole('button', { name: 'Logout' }).click();
  await page.waitForURL('**/auth/signin');
}
