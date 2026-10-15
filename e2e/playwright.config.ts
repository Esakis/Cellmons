import { defineConfig, devices } from '@playwright/test';

// Testy działają na zbudowanym kliencie (`pnpm --filter web build`), serwowanym przez `vite preview`.
const baseURL = 'http://localhost:4173';
const ci = Boolean(process.env.CI);

export default defineConfig({
  testDir: './tests',
  forbidOnly: ci,
  retries: ci ? 2 : 0,
  workers: ci ? 2 : undefined,
  reporter: ci ? [['github'], ['html', { open: 'never' }]] : 'list',
  globalTeardown: './teardown.ts',
  use: {
    baseURL,
    trace: 'retain-on-failure',
  },
  projects: [
    { name: 'chromium', use: { ...devices['Desktop Chrome'] } },
    { name: 'mobile-chromium', use: { ...devices['Pixel 5'] } },
  ],
  webServer: [
    {
      command: 'cargo run --quiet -p server --example browser_fixture --locked',
      cwd: '..',
      url: 'http://127.0.0.1:18092/healthz',
      timeout: 180000,
      reuseExistingServer: false,
    },
    {
      command: 'pnpm --filter web exec vite preview --config vite.e2e.config.ts',
      url: baseURL,
      reuseExistingServer: false,
    },
  ],
});
