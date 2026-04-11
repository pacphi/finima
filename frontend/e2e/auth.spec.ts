import { test, expect } from '@playwright/test';
import { requestMagicLink, verifyMagicLink, signIn, signOut } from './helpers/auth';
import { testUserEmail } from './helpers/fixtures';

test.beforeEach(async () => {
  if (!process.env.E2E_ENABLED) {
    test.skip();
  }
});

test.describe('Authentication flow', () => {
  test('should show sign-in page with email input and submit button', async ({ page }) => {
    await page.goto('/auth/signin');

    await expect(page.getByRole('heading', { name: 'Finima' })).toBeVisible();
    await expect(page.getByPlaceholder('Email address')).toBeVisible();
    await expect(page.getByRole('button', { name: 'Send Magic Link' })).toBeVisible();
  });

  test('should submit email and navigate to magic-link-sent page', async ({ page }) => {
    const email = testUserEmail();
    await page.goto('/auth/signin');

    await page.getByPlaceholder('Email address').fill(email);
    await page.getByRole('button', { name: 'Send Magic Link' }).click();

    await page.waitForURL('**/auth/magic-link-sent');
    await expect(page.getByText('Check your email')).toBeVisible();
    await expect(page.getByText(email)).toBeVisible();
  });

  test('should show validation error for invalid email', async ({ page }) => {
    await page.goto('/auth/signin');

    await page.getByPlaceholder('Email address').fill('not-an-email');
    await page.getByRole('button', { name: 'Send Magic Link' }).click();

    await expect(page.getByText('Please enter a valid email address')).toBeVisible();
  });

  test('should complete sign-in via magic link verification', async ({ page, request }) => {
    const email = testUserEmail();
    await signIn(page, request, email);

    // New users go to onboarding, existing users go to dashboard.
    // Either destination confirms authentication succeeded.
    await expect(page).toHaveURL(/\/(dashboard|onboarding)/);
  });

  test('should persist session across page refresh', async ({ page, request }) => {
    const email = testUserEmail();
    await signIn(page, request, email);

    // Wait for the app to load
    await expect(page).toHaveURL(/\/(dashboard|onboarding)/);

    // Reload the page and verify we remain authenticated
    await page.reload();
    await expect(page).toHaveURL(/\/(dashboard|onboarding)/);

    // Should not be redirected to sign-in
    await expect(page.url()).not.toContain('/auth/signin');
  });

  test('should log out and redirect to sign-in', async ({ page, request }) => {
    const email = testUserEmail();
    await signIn(page, request, email);
    await expect(page).toHaveURL(/\/(dashboard|onboarding)/);

    // If we land on onboarding, complete it first so we get the header with Logout
    if (page.url().includes('/onboarding')) {
      // Fill profile step
      await page.getByPlaceholder('Your name').fill('Logout Test');
      await page.getByRole('button', { name: 'Next' }).click();

      // Fill portfolio step (defaults are fine)
      await page.getByRole('button', { name: 'Next' }).click();

      // Skip account step to get to the main app
      await page.getByRole('button', { name: 'Skip' }).click();
      await page.waitForURL('**/accounts');
    }

    await signOut(page);
    await expect(page).toHaveURL(/\/auth\/signin/);

    // Verify that navigating to a protected route redirects back to sign-in
    await page.goto('/dashboard');
    await expect(page).toHaveURL(/\/auth\/signin/);
  });
});
