import { expect, test } from '@playwright/test';

test('recreates the worker after a transient WASM download failure', async ({ page, context }) => {
  await page.addInitScript(() => localStorage.setItem('game.language', 'en'));
  await context.route('**/*.wasm', (route) => route.abort());
  await page.goto('/#/duel?avatar=1&bot=novice');
  await expect(
    page.getByText('The engine did not respond. Return to avatar selection and try again.'),
  ).toBeVisible();
  await context.unroute('**/*.wasm');
  await page.getByRole('button', { name: 'Try again' }).click();
  await expect(page.getByRole('button', { name: 'Confirm move' })).toBeEnabled();
});
