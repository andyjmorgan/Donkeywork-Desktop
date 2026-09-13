import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: '.',
  testMatch: 'web-console.spec.mjs',
  outputDir: '../../artifacts/web-tests',
  workers: 1,
  use: {
    baseURL: 'http://127.0.0.1:5178',
    viewport: { width: 1280, height: 900 },
    launchOptions: process.env.DESKTOP_TEST_CHROMIUM
      ? { executablePath: process.env.DESKTOP_TEST_CHROMIUM } : {},
  },
  webServer: {
    command: 'npm --prefix web run dev -- --port 5178 --strictPort',
    cwd: '../..',
    url: 'http://127.0.0.1:5178',
    reuseExistingServer: false,
  },
});
