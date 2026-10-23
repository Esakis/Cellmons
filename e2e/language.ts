import type { Page } from '@playwright/test';

// The language is chosen in Settings; the test then returns to the page it was on.
export async function chooseLanguage(page: Page, language: 'pl' | 'en') {
  await page.locator('header nav a[href$="/settings"]').click();
  await page.locator('.settings-page select').first().selectOption(language);
  await page.goBack();
}
