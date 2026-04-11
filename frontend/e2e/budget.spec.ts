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

test.describe('Budget management', () => {
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
          display_name: 'Budget Test User',
          default_currency: 'USD',
          date_format: 'MM/DD/YYYY',
        },
      });
    }

    const portfolio = await createPortfolioViaApi(request, accessToken, TEST_PORTFOLIO_NAME);
    await createAccountViaApi(request, accessToken, portfolio.id);

    await signIn(page, request, email);
  });

  test('should display the budget page with controls', async ({ page }) => {
    await page.goto('/budget');

    await expect(page.getByRole('heading', { name: 'Budget' })).toBeVisible();
    await expect(page.getByRole('button', { name: 'Auto-Suggest Budget' })).toBeVisible();
    await expect(page.getByRole('button', { name: '+ New Budget Entry' })).toBeVisible();
    await expect(page.getByRole('button', { name: 'Prev' })).toBeVisible();
    await expect(page.getByRole('button', { name: 'Next' })).toBeVisible();
  });

  test('should create a new budget entry', async ({ page }) => {
    await page.goto('/budget');

    // Click new budget entry button
    await page.getByRole('button', { name: '+ New Budget Entry' }).click();

    // Fill in the new budget form
    await expect(page.getByText('New Budget Entry')).toBeVisible();
    await page.getByPlaceholder('Category').fill('Groceries');
    await page.getByPlaceholder('Limit').fill('500');
    await page.getByRole('button', { name: 'Create' }).click();

    // The budget table should now show the new entry
    await expect(page.getByText('Groceries')).toBeVisible({ timeout: 5_000 });
  });

  test('should edit an existing budget limit', async ({ page }) => {
    await page.goto('/budget');

    // First create a budget entry
    await page.getByRole('button', { name: '+ New Budget Entry' }).click();
    await page.getByPlaceholder('Category').fill('Dining');
    await page.getByPlaceholder('Limit').fill('300');
    await page.getByRole('button', { name: 'Create' }).click();

    await expect(page.getByText('Dining')).toBeVisible({ timeout: 5_000 });

    // Click Edit on the entry
    await page.getByRole('button', { name: 'Edit' }).first().click();

    // Change the limit
    const limitInput = page.locator('input[type="number"]').last();
    await limitInput.clear();
    await limitInput.fill('400');

    // Save
    await page.getByRole('button', { name: 'Save' }).click();

    // Verify the table reloads (the edit button should reappear)
    await expect(page.getByRole('button', { name: 'Edit' })).toBeVisible({ timeout: 5_000 });
  });

  test('should display budget progress bar for categories', async ({ page }) => {
    await page.goto('/budget');

    // Create a budget entry so we have something to display
    await page.getByRole('button', { name: '+ New Budget Entry' }).click();
    await page.getByPlaceholder('Category').fill('Transport');
    await page.getByPlaceholder('Limit').fill('200');
    await page.getByRole('button', { name: 'Create' }).click();

    // The budget table should contain headers
    await expect(page.getByText('Category')).toBeVisible();
    await expect(page.getByText('Budget', { exact: false })).toBeVisible();
    await expect(page.getByText('Spent')).toBeVisible();
    await expect(page.getByText('Remaining')).toBeVisible();
    await expect(page.getByText('Progress')).toBeVisible();
  });

  test('should navigate between months', async ({ page }) => {
    await page.goto('/budget');

    // Get the current month text
    const monthText = page.locator('span').filter({ hasText: /\w+ \d{4}/ });
    const initialMonth = await monthText.textContent();

    // Click Prev
    await page.getByRole('button', { name: 'Prev' }).click();
    await expect(monthText).not.toHaveText(initialMonth!);

    // Click Next twice to go forward
    await page.getByRole('button', { name: 'Next' }).click();
    await expect(monthText).toHaveText(initialMonth!);
  });

  test('should show savings goals section', async ({ page }) => {
    await page.goto('/budget');

    await expect(page.getByRole('heading', { name: 'Savings Goals' })).toBeVisible();
    await expect(page.getByRole('button', { name: '+ New Goal' })).toBeVisible();
  });
});
