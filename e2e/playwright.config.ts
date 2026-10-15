import { defineConfig, devices } from '@playwright/test';

// Testy działają na zbudowanym kliencie (`pnpm --filter web build`), serwowanym przez `vite preview`.
const baseURL = 'http://localhost:4173';
const ci = Boolean(process.env.CI);

export default defineConfig({
  testDir: './tests',
  forbidOnly: ci,
  retries: ci ? 2 : 0,
  reporter: ci ? [['github'], ['html', { open: 'never' }]] : 'list',
  use: {
    baseURL,
    trace: 'on-first-retry',
  },
  projects: [
    { name: 'chromium', use: { ...devices['Desktop Chrome'] } },
    { name: 'mobile-chromium', use: { ...devices['Pixel 5'] } },
  ],
  webServer: {
    command: 'pnpm --filter web preview',
    url: baseURL,
    reuseExistingServer: !ci,
  },
});
