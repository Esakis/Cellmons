import { expect, test } from '@playwright/test';
import { accessible } from '../accessibility';
import { chooseLanguage } from '../language';

test('public legal documents, licenses and deletion route work without an installed app', async ({
  page,
}) => {
  await page.addInitScript(() => localStorage.setItem('game.language', 'pl'));
  let api = 0;
  page.on('request', (request) => {
    if (new URL(request.url()).pathname.startsWith('/api/')) api++;
  });
  await page.goto('/legal/privacy.html');
  await expect(
    page.getByRole('heading', {
      name: 'Polityka prywatności Spellgambit: Arcane Class',
      exact: true,
    }),
  ).toBeVisible();
  await accessible(page);
  await page.goto('/#/legal?doc=privacy');
  await expect(
    page.getByRole('heading', {
      name: 'Polityka prywatności Spellgambit: Arcane Class',
      exact: true,
    }),
  ).toBeVisible();
  expect(api).toBe(0);
  await chooseLanguage(page, 'en');
  await page
    .getByRole('link', { name: 'Delete a Spellgambit: Arcane Class account', exact: true })
    .click();
  await expect(
    page.getByRole('heading', { name: 'Delete a Spellgambit: Arcane Class account', exact: true }),
  ).toBeVisible();
  await page.getByText('Font licenses', { exact: true }).click();
  await expect(
    page
      .locator('details')
      .filter({ has: page.getByText('Font licenses', { exact: true }) })
      .locator('pre'),
  ).toHaveCount(2);
  await page.getByText('Software licenses', { exact: true }).click();
  await expect(page.locator('.legal-page pre').last()).toContainText(
    'Rust standard library 1.99.0',
  );
  expect(api).toBe(0);
  await accessible(page);
  await page.getByRole('link', { name: 'Open the data panel', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Your data', exact: true })).toBeVisible();
  await expect(
    page.getByRole('heading', { name: 'Access your data without gameplay', exact: true }),
  ).toBeVisible();
});
