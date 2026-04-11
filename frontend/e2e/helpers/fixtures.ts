import { type APIRequestContext } from '@playwright/test';

const API_BASE = process.env.E2E_API_BASE_URL || 'http://localhost:3000';

/** Unique test user email seeded with a timestamp to avoid collisions. */
export function testUserEmail(): string {
  const id = Date.now().toString(36) + Math.random().toString(36).slice(2, 6);
  return `e2e-${id}@test.finima.local`;
}

/** A reusable email for tests that need a stable, pre-existing user. */
export const STABLE_TEST_EMAIL = 'e2e-stable@test.finima.local';

export const TEST_PORTFOLIO_NAME = 'E2E Test Portfolio';

export const TEST_ACCOUNT = {
  name: 'E2E Checking',
  type: 'checking',
  institution: 'Test Bank',
  openingBalance: 1000,
} as const;

export const TEST_DISPLAY_NAME = 'E2E User';
export const TEST_CURRENCY = 'USD';
export const TEST_DATE_FORMAT = 'MM/DD/YYYY';

/**
 * Create an account via the API for a signed-in user.
 * Returns the created account object.
 */
export async function createAccountViaApi(
  request: APIRequestContext,
  accessToken: string,
  portfolioId: string,
  overrides: Partial<{
    name: string;
    account_type: string;
    institution: string;
    currency: string;
    opening_balance: number;
  }> = {},
): Promise<{ id: string; name: string }> {
  const response = await request.post(`${API_BASE}/api/accounts`, {
    headers: { Authorization: `Bearer ${accessToken}` },
    data: {
      portfolio_id: portfolioId,
      name: overrides.name ?? TEST_ACCOUNT.name,
      account_type: overrides.account_type ?? TEST_ACCOUNT.type,
      institution: overrides.institution ?? TEST_ACCOUNT.institution,
      currency: overrides.currency ?? TEST_CURRENCY,
      opening_balance: overrides.opening_balance ?? TEST_ACCOUNT.openingBalance,
    },
  });
  return response.json();
}

/**
 * Create a portfolio via the API for a signed-in user.
 * Returns the created portfolio object.
 */
export async function createPortfolioViaApi(
  request: APIRequestContext,
  accessToken: string,
  name = TEST_PORTFOLIO_NAME,
): Promise<{ id: string; name: string }> {
  const response = await request.post(`${API_BASE}/api/portfolios`, {
    headers: { Authorization: `Bearer ${accessToken}` },
    data: { name },
  });
  return response.json();
}
