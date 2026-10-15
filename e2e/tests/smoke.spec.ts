import { readFileSync } from 'node:fs';
import { expect, test } from '@playwright/test';

test('strona startowa pokazuje postacie, boty i przełącza język', async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem('game.language', 'pl'));
  await page.goto('/');
  await expect(page).toHaveTitle('Spellgambit: Arcane Class');
  await expect(page.getByRole('heading', { level: 1 })).toHaveText('Twój ruch. Twoja magia.');
  const roster = JSON.parse(
    readFileSync(new URL('../../web/src/generated/content.json', import.meta.url), 'utf8'),
  ) as { avatars: unknown[] };
  await expect(page.locator('button[aria-pressed]')).toHaveCount(roster.avatars.length + 4);
  await expect(page.locator('button[aria-pressed="false"]')).toHaveCount(roster.avatars.length + 2);
  await page.getByRole('combobox', { name: 'Język' }).selectOption('en');
  await expect(page.getByRole('heading', { level: 1 })).toHaveText('Your move. Your magic.');
});
