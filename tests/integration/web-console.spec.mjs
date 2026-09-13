import { test, expect } from '@playwright/test';

test('demo controls distinguish viewing scale from simulated source mode', async ({ page }) => {
  const failures = [];
  page.on('pageerror', error => failures.push(error.message));
  await page.goto('/');
  await expect(page.getByText('Keycloak not connected')).toBeVisible();
  await expect(page.getByRole('button', { name: 'Sign in', exact: true })).toBeDisabled();
  await expect(page.getByRole('button', { name: 'Register device' })).toBeDisabled();
  if (process.env.DESKTOP_TEST_SCREENSHOTS === '1') {
    await page.screenshot({ path: 'artifacts/web-console/dark-preview.png' });
  }
  await page.getByRole('button', { name: 'Native 1:1' }).click();
  await expect(page.getByRole('button', { name: 'Native 1:1' })).toHaveAttribute('aria-pressed', 'true');
  await expect(page.getByRole('img', { name: /Local test pattern at simulated 3840/ })).toBeAttached();
  await page.getByRole('button', { name: 'Fit', exact: true }).click();
  await page.getByLabel('Supported demo resolution').selectOption('1');
  await page.getByRole('button', { name: 'Apply demo mode' }).click();
  await expect(page.getByText(/Simulated resolution changed to 1920/)).toBeVisible();
  await page.getByText('Demo behaviour', { exact: true }).click();
  await page.getByRole('checkbox', { name: /Simulate the next/ }).check();
  await page.getByLabel('Supported demo resolution').selectOption('0');
  await page.getByRole('button', { name: 'Apply demo mode' }).click();
  await expect(page.getByText(/Simulated change failed. Preview remains 1920/)).toBeVisible();
  await page.getByRole('button', { name: 'Close preview', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Preview closed' })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Open local preview', exact: true })).toBeFocused();
  await page.getByRole('button', { name: 'Open local preview', exact: true }).click();
  await expect(page.getByRole('img', { name: /Local test pattern at simulated 1920/ })).toBeVisible();
  expect(failures).toEqual([]);
});

test('mobile fixture has no page-level horizontal overflow', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto('/');
  await expect(page.getByRole('heading', { name: 'Spark', exact: true })).toBeVisible();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
});

test('DonkeyWork theme preference persists and fullscreen exits cleanly', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Switch to light mode' }).click();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
  await page.reload();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
  if (process.env.DESKTOP_TEST_SCREENSHOTS === '1') {
    await page.screenshot({ path: 'artifacts/web-console/light-preview.png' });
  }
  await page.getByRole('button', { name: 'Fullscreen preview', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Exit fullscreen preview' })).toBeVisible();
  await page.getByRole('button', { name: 'Exit fullscreen preview' }).click();
  await expect.poll(() => page.evaluate(() => document.fullscreenElement === null)).toBe(true);
});
