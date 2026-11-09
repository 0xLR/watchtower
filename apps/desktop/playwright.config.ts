import { defineConfig } from '@playwright/test';
export default defineConfig({
  timeout: 60000, testDir: './e2e', testMatch: '**/*.spec.ts', fullyParallel: false, workers: 1,
  use: { baseURL: 'http://127.0.0.1:1420', viewport: { width: 1440, height: 1000 }, headless: true },
  webServer: { command: 'pnpm dev', url: 'http://127.0.0.1:1420', reuseExistingServer: !process.env.CI },
});
