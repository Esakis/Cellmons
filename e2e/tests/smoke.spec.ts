import { expect, test } from '@playwright/test';

test('strona startowa pokazuje nazwę gry', async ({ page }) => {
  await page.goto('/');
  await expect(page).toHaveTitle('Spellgambit: Arcane Class');
  await expect(page.getByRole('heading', { level: 1 })).toHaveText('Spellgambit: Arcane Class');
});
