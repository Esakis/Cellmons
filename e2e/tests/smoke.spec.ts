import { readFileSync } from 'node:fs';
import { expect, test } from '@playwright/test';
import { accessible } from '../accessibility';

test('strona startowa pokazuje postacie, boty i przełącza język', async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem('game.language', 'pl'));
  await page.goto('/');
  await expect(page).toHaveTitle('Spellgambit: Arcane Class');
  const initialURL = page.url();
  await page.keyboard.press('Tab');
  await expect(page.getByRole('button', { name: 'Przejdź do treści', exact: true })).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(page.locator('main')).toBeFocused();
  expect(page.url()).toBe(initialURL);
  await accessible(page);
  await expect(page.getByRole('heading', { level: 1 })).toHaveText('Twój ruch. Twoja magia.');
  const roster = JSON.parse(
    readFileSync(new URL('../../web/src/generated/content.json', import.meta.url), 'utf8'),
  ) as { avatars: unknown[] };
  await expect(page.locator('button[aria-pressed]')).toHaveCount(roster.avatars.length + 4);
  await expect(page.locator('button[aria-pressed="false"]')).toHaveCount(roster.avatars.length + 2);
  await page.getByRole('combobox', { name: 'Język' }).selectOption('en');
  await expect(page.getByRole('heading', { level: 1 })).toHaveText('Your move. Your magic.');
});
