import { expect, test } from '@playwright/test';

test('dark theme supports explicit card actions and touch without page overflow', async ({
  page,
  isMobile,
}) => {
  await page.emulateMedia({ colorScheme: 'dark' });
  await page.addInitScript(() => {
    localStorage.setItem('game.language', 'en');
    localStorage.setItem('game.speed', 'instant');
  });
  await page.goto('/#/duel?avatar=3&bot=tactician');
  const confirm = page.getByRole('button', { name: 'Confirm move' });
  await expect(confirm).toBeEnabled();
  const discard = page.getByRole('button', { name: 'Discard', exact: true }).first();
  if (isMobile) await discard.tap();
  else await discard.click();
  await expect(discard).toHaveAttribute('aria-pressed', 'true');
  await expect(confirm).toBeEnabled();
  const details = page.getByRole('button', { name: /^Details:/ }).first();
  if (isMobile) await details.tap();
  else await details.click();
  await expect(page.getByRole('dialog')).toBeVisible();
  await page.getByRole('button', { name: 'Close', exact: true }).last().click();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  if (isMobile) await confirm.tap();
  else await confirm.click();
  await expect(confirm.or(page.getByRole('button', { name: 'Play again' }))).toBeEnabled();
});
