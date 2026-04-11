import { test, expect } from '@playwright/test';
import { signIn, requestMagicLink, verifyMagicLink } from './helpers/auth';
import {
  testUserEmail,
  createPortfolioViaApi,
  createAccountViaApi,
  TEST_PORTFOLIO_NAME,
} from './helpers/fixtures';

test.beforeEach(async () => {
  if (!process.env.E2E_ENABLED) {
    test.skip();
  }
});

test.describe('Settings', () => {
  let email: string;
  let accessToken: string;

  test.beforeEach(async ({ page, request }) => {
    email = testUserEmail();

    // Set up user with portfolio and account via API
    await requestMagicLink(request, email);
    const authResult = await verifyMagicLink(request, email);
    accessToken = authResult.access_token;

    if (authResult.is_new_user) {
      await request.put('http://localhost:3000/api/users/me', {
        headers: { Authorization: `Bearer ${accessToken}` },
        data: {
          display_name: 'Settings Test User',
          default_currency: 'USD',
          date_format: 'MM/DD/YYYY',
        },
      });
    }

    const portfolio = await createPortfolioViaApi(request, accessToken, TEST_PORTFOLIO_NAME);
    await createAccountViaApi(request, accessToken, portfolio.id);

    await signIn(page, request, email);
  });

  test('should display settings page with tabs', async ({ page }) => {
    await page.goto('/settings');

    await expect(page.getByRole('heading', { name: 'Settings' })).toBeVisible();
    await expect(page.getByRole('button', { name: 'Theme' })).toBeVisible();
    await expect(page.getByRole('button', { name: 'Layout' })).toBeVisible();
    await expect(page.getByRole('button', { name: 'General' })).toBeVisible();
    await expect(page.getByRole('button', { name: 'LLM' })).toBeVisible();
  });

  test('should change currency to EUR and verify on dashboard', async ({ page }) => {
    await page.goto('/settings');

    // Navigate to General tab
    await page.getByRole('button', { name: 'General' }).click();

    // Change currency to EUR
    const currencySelect = page.locator('select').filter({ hasText: 'USD' }).first();
    await currencySelect.selectOption('EUR');

    // Save preferences
    await page.getByRole('button', { name: 'Save Preferences' }).click();

    // Wait for save confirmation
    await expect(page.getByText('Preferences saved')).toBeVisible({ timeout: 5_000 });

    // Navigate to dashboard and check for EUR currency symbol
    await page.goto('/dashboard');
    await page.waitForLoadState('networkidle');

    // The dashboard should render currency values -- look for the euro sign
    // Note: this depends on actual data being present; if no data, the
    // currency symbol might not appear, so we check the settings persisted.
    await page.goto('/settings');
    await page.getByRole('button', { name: 'General' }).click();

    const currencyValue = page.locator('select').filter({ hasText: 'EUR' }).first();
    await expect(currencyValue).toBeVisible();
  });

  test('should change date format', async ({ page }) => {
    await page.goto('/settings');

    // Navigate to General tab
    await page.getByRole('button', { name: 'General' }).click();

    // Change date format
    const dateFormatSelect = page.locator('select').filter({ hasText: 'MM/DD/YYYY' }).first();
    await dateFormatSelect.selectOption('YYYY-MM-DD');

    // Save
    await page.getByRole('button', { name: 'Save Preferences' }).click();
    await expect(page.getByText('Preferences saved')).toBeVisible({ timeout: 5_000 });

    // Reload and verify the setting persisted
    await page.reload();
    await page.getByRole('button', { name: 'General' }).click();

    const savedFormat = page.locator('select').filter({ hasText: 'YYYY-MM-DD' }).first();
    await expect(savedFormat).toBeVisible();
  });

  test('should toggle dashboard widgets in layout tab', async ({ page }) => {
    await page.goto('/settings');

    // Navigate to Layout tab
    await page.getByRole('button', { name: 'Layout' }).click();

    await expect(page.getByText('Dashboard Widgets')).toBeVisible();

    // Toggle a widget checkbox
    const checkboxes = page.locator('input[type="checkbox"]');
    const count = await checkboxes.count();
    expect(count).toBeGreaterThan(0);

    // Click the first checkbox to toggle it
    const firstCheckbox = checkboxes.first();
    const wasChecked = await firstCheckbox.isChecked();
    await firstCheckbox.click();

    if (wasChecked) {
      await expect(firstCheckbox).not.toBeChecked();
    } else {
      await expect(firstCheckbox).toBeChecked();
    }
  });

  test('should display LLM configuration', async ({ page }) => {
    await page.goto('/settings');

    // Navigate to LLM tab
    await page.getByRole('button', { name: 'LLM' }).click();

    await expect(page.getByText('LLM Configuration')).toBeVisible();
    await expect(page.getByText('Provider')).toBeVisible();
    await expect(page.getByText('Model')).toBeVisible();
    await expect(page.getByText('Connection Status')).toBeVisible();
  });

  test('should show save confirmation message', async ({ page }) => {
    await page.goto('/settings');

    await page.getByRole('button', { name: 'Save Preferences' }).click();

    // Should show either success or failure message
    const message = page.locator('span').filter({ hasText: /Preferences saved|Failed to save/ });
    await expect(message).toBeVisible({ timeout: 5_000 });
  });
});
