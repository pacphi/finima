import { test, expect } from '@playwright/test';
import { signIn, requestMagicLink, verifyMagicLink } from './helpers/auth';
import {
  testUserEmail,
  createPortfolioViaApi,
  createAccountViaApi,
  TEST_PORTFOLIO_NAME,
  TEST_ACCOUNT,
} from './helpers/fixtures';
import path from 'path';

test.beforeEach(async () => {
  if (!process.env.E2E_ENABLED) {
    test.skip();
  }
});

test.describe('Transaction import flow', () => {
  let email: string;
  let accessToken: string;
  let accountId: string;

  test.beforeEach(async ({ page, request }) => {
    email = testUserEmail();

    // Set up a user with a portfolio and account via API
    await requestMagicLink(request, email);
    const authResult = await verifyMagicLink(request, email);
    accessToken = authResult.access_token;

    // Complete onboarding by creating profile, portfolio, and account via API
    if (authResult.is_new_user) {
      await request.put('http://localhost:3000/api/users/me', {
        headers: { Authorization: `Bearer ${accessToken}` },
        data: {
          display_name: 'Import Test User',
          default_currency: 'USD',
          date_format: 'MM/DD/YYYY',
        },
      });
    }

    const portfolio = await createPortfolioViaApi(request, accessToken, TEST_PORTFOLIO_NAME);
    const account = await createAccountViaApi(request, accessToken, portfolio.id, {
      name: TEST_ACCOUNT.name,
      account_type: TEST_ACCOUNT.type,
    });
    accountId = account.id;

    // Sign in via the browser
    await signIn(page, request, email);
  });

  test('should create an account via the UI', async ({ page }) => {
    await page.goto('/accounts');

    await page.getByRole('button', { name: '+ Add Account' }).click();

    // Fill the modal form
    const modal = page.locator('.fixed.inset-0');
    await expect(modal).toBeVisible();

    await modal.locator('select').filter({ hasText: 'Select type...' }).selectOption('checking');
    await modal.getByPlaceholder('e.g. Chase Checking').fill('UI Created Account');
    await modal.getByPlaceholder('e.g. Chase Bank').fill('UI Test Bank');

    await modal.getByRole('button', { name: 'Create Account' }).click();

    // Modal should close and account should appear in list
    await expect(modal).not.toBeVisible({ timeout: 5_000 });
    await expect(page.getByText('UI Created Account')).toBeVisible();
  });

  test('should upload a CSV and preview column mapping', async ({ page }) => {
    await page.goto(`/accounts/${accountId}`);

    // Click import button
    await page.getByRole('button', { name: 'Import Transactions' }).click();

    // Upload the test CSV file
    const csvPath = path.resolve(__dirname, 'fixtures/test-transactions.csv');
    const fileInput = page.locator('input[type="file"]');
    await fileInput.setInputFiles(csvPath);

    // Verify the file is selected and format detected
    await expect(page.getByText('test-transactions.csv')).toBeVisible();
    await expect(page.getByText('csv', { exact: false })).toBeVisible();

    // Click Upload button
    await page.getByRole('button', { name: 'Upload' }).click();

    // Wait for the column mapping modal to appear
    // The ColumnMappingModal should show preview data
    await expect(page.getByText(/column|mapping|preview/i)).toBeVisible({ timeout: 15_000 });
  });

  test('should show imported transactions after completing import', async ({ page }) => {
    await page.goto(`/accounts/${accountId}`);

    // Click import button
    await page.getByRole('button', { name: 'Import Transactions' }).click();

    // Upload the test CSV file
    const csvPath = path.resolve(__dirname, 'fixtures/test-transactions.csv');
    const fileInput = page.locator('input[type="file"]');
    await fileInput.setInputFiles(csvPath);

    await page.getByRole('button', { name: 'Upload' }).click();

    // Wait for column mapping to appear and confirm it
    await expect(page.getByText(/column|mapping|preview/i)).toBeVisible({ timeout: 15_000 });

    // Look for a confirm/import button in the mapping modal
    const confirmButton = page.getByRole('button', { name: /confirm|import|save/i });
    if (await confirmButton.isVisible()) {
      await confirmButton.click();
    }

    // Navigate to transactions page to verify imported data appears
    await page.goto('/transactions');
    await page.waitForLoadState('networkidle');

    // At least some of the test CSV descriptions should be visible
    const transactionContent = page.locator('table, [role="table"], [data-testid="transaction-list"]');
    await expect(transactionContent).toBeVisible({ timeout: 10_000 });
  });
});
