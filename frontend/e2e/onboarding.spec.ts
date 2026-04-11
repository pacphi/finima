import { test, expect } from '@playwright/test';
import { signIn } from './helpers/auth';
import {
  testUserEmail,
  TEST_PORTFOLIO_NAME,
  TEST_DISPLAY_NAME,
} from './helpers/fixtures';

test.beforeEach(async () => {
  if (!process.env.E2E_ENABLED) {
    test.skip();
  }
});

test.describe('Onboarding flow', () => {
  test('should complete full onboarding as a new user', async ({ page, request }) => {
    const email = testUserEmail();
    await signIn(page, request, email);

    // New users should land on onboarding
    // If the user happens to already exist, skip this test
    if (!page.url().includes('/onboarding')) {
      test.skip();
      return;
    }

    await expect(page.getByText('Welcome to Finima')).toBeVisible();
    await expect(page.getByText('Step 1 of 3')).toBeVisible();

    // Step 1: Profile
    await expect(page.getByText('Set Up Your Profile')).toBeVisible();
    await page.getByPlaceholder('Your name').fill(TEST_DISPLAY_NAME);
    // Select EUR currency
    await page.locator('select').filter({ hasText: 'USD' }).first().selectOption('EUR');
    // Select date format
    await page.locator('select').filter({ hasText: 'MM/DD/YYYY' }).first().selectOption('YYYY-MM-DD');
    await page.getByRole('button', { name: 'Next' }).click();

    // Step 2: Portfolio
    await expect(page.getByText('Create Your Portfolio')).toBeVisible();
    await expect(page.getByText('Step 2 of 3')).toBeVisible();

    const portfolioInput = page.locator('input[placeholder="My Finances"]');
    await portfolioInput.clear();
    await portfolioInput.fill(TEST_PORTFOLIO_NAME);
    await page.getByRole('button', { name: 'Next' }).click();

    // Step 3: Account
    await expect(page.getByText('Add Your First Account')).toBeVisible();
    await expect(page.getByText('Step 3 of 3')).toBeVisible();

    await page.locator('select').filter({ hasText: 'Checking' }).first().selectOption('checking');
    await page.getByPlaceholder('e.g. Chase Checking').fill('Test Checking');
    await page.getByPlaceholder('e.g. Chase Bank').fill('Test Bank');
    await page.getByRole('button', { name: 'Complete Setup' }).click();

    // Should redirect to account detail page after completion
    await expect(page).toHaveURL(/\/accounts\/.+/, { timeout: 10_000 });
  });

  test('should allow skipping account creation in onboarding', async ({ page, request }) => {
    const email = testUserEmail();
    await signIn(page, request, email);

    if (!page.url().includes('/onboarding')) {
      test.skip();
      return;
    }

    // Step 1: Profile
    await page.getByPlaceholder('Your name').fill('Skip Test User');
    await page.getByRole('button', { name: 'Next' }).click();

    // Step 2: Portfolio (use defaults)
    await page.getByRole('button', { name: 'Next' }).click();

    // Step 3: Skip account creation
    await page.getByRole('button', { name: 'Skip' }).click();

    // Should redirect to accounts page
    await expect(page).toHaveURL(/\/accounts$/, { timeout: 10_000 });
  });

  test('should allow navigating back through onboarding steps', async ({ page, request }) => {
    const email = testUserEmail();
    await signIn(page, request, email);

    if (!page.url().includes('/onboarding')) {
      test.skip();
      return;
    }

    // Step 1: Fill and proceed
    await page.getByPlaceholder('Your name').fill('Back Test');
    await page.getByRole('button', { name: 'Next' }).click();

    // Step 2: Go back
    await expect(page.getByText('Create Your Portfolio')).toBeVisible();
    await page.getByRole('button', { name: 'Back' }).click();

    // Should be back on Step 1 with data preserved
    await expect(page.getByText('Set Up Your Profile')).toBeVisible();
    await expect(page.getByPlaceholder('Your name')).toHaveValue('Back Test');
  });
});
