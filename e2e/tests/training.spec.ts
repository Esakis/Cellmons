import { expect, test } from '@playwright/test';

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    localStorage.setItem('game.language', 'en');
    localStorage.setItem('game.speed', 'instant');
  });
});

test('plays a full human training duel with keyboard, cost preview and card inspection', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (error) => errors.push(error.message));
  await page.goto('/');
  await page.getByRole('link', { name: 'Start training' }).click();
  const confirm = page.getByRole('button', { name: 'Confirm move' });
  await expect(confirm).toBeEnabled();
  await expect(page.getByRole('heading', { level: 1 })).toHaveText('Round 1 / 5');
  await page.keyboard.press('1');
  await expect(page.getByText('1 K · 3 K remaining')).toBeVisible();
  await page.keyboard.press('Shift+Q');
  await expect(page.getByRole('dialog')).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(page.getByRole('dialog')).toHaveCount(0);
  await page.keyboard.press('q');
  await expect(confirm).toBeEnabled();
  // Native Enter on the focused slot must retain normal button behavior.
  await confirm.focus();
  await page.keyboard.press('Enter');
  await expect(page.getByRole('button', { name: 'Replay round' })).toBeEnabled();
  for (let round = 0; round < 4; round++) {
    if (await page.locator('#duel-result').count()) break;
    await expect(confirm).toBeEnabled();
    await confirm.click();
    await expect(confirm.or(page.getByRole('button', { name: 'Play again' }))).toBeEnabled();
  }
  await expect(page.locator('#duel-result')).toBeVisible();
  await page.getByText('Round log', { exact: true }).click();
  await expect(page.getByText('Duel ended.', { exact: true })).toBeVisible();
  expect(errors).toEqual([]);
});

test('precache includes the unopened WASM worker: complete bot duel after offline reload', async ({
  page,
  context,
}) => {
  await page.goto('/');
  await expect(page.getByTestId('offline-status')).toHaveText('Ready to play offline');
  await page.evaluate(async () => {
    await navigator.serviceWorker.ready;
    if (!navigator.serviceWorker.controller)
      await new Promise<void>((resolve) =>
        navigator.serviceWorker.addEventListener('controllerchange', () => resolve(), {
          once: true,
        }),
      );
  });
  await context.setOffline(true);
  await page.reload();
  await expect(page.getByRole('heading', { level: 1 })).toHaveText('Your move. Your magic.');
  await page.getByRole('link', { name: 'Watch bots play' }).click();
  const next = page.getByRole('button', { name: 'Next bot round' });
  for (let round = 0; round < 5; round++) {
    if (await page.locator('#duel-result').count()) break;
    await expect(next).toBeEnabled();
    await next.click();
    await expect(next.or(page.getByRole('button', { name: 'Play again' }))).toBeEnabled();
  }
  await expect(page.locator('#duel-result')).toBeVisible();
});

test('forfeit confirmation preserves the duel on cancel and confirms navigation', async ({
  page,
}) => {
  await page.goto('/#/duel?avatar=6&bot=archmage');
  await expect(page.getByRole('button', { name: 'Confirm move' })).toBeEnabled();
  await page.getByRole('link', { name: 'Settings', exact: true }).click();
  await expect(page.getByRole('dialog')).toBeVisible();
  await page.getByRole('button', { name: 'Return to duel' }).click();
  await expect(page.getByRole('heading', { level: 1 })).toHaveText('Round 1 / 5');
  await page.getByRole('link', { name: 'Settings', exact: true }).click();
  await page.getByRole('dialog').getByRole('button', { name: 'Forfeit duel' }).click();
  await expect(page.getByRole('heading', { level: 1 })).toHaveText('Settings');
});

test('animation can be skipped and the last round replayed', async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem('game.speed', 'normal'));
  await page.goto('/');
  await page.getByRole('link', { name: 'Start training' }).click();
  await page.getByRole('button', { name: 'Confirm move' }).click();
  await expect(page.getByRole('button', { name: 'Skip animation' })).toBeVisible();
  await page.keyboard.press('Space');
  await expect(page.getByRole('button', { name: 'Confirm move' })).toBeEnabled();
  await page.getByRole('button', { name: 'Replay round' }).click();
  await expect(page.getByRole('button', { name: 'Skip animation' })).toBeVisible();
  await page.getByRole('button', { name: 'Skip animation' }).click();
  await expect(page.getByRole('button', { name: 'Confirm move' })).toBeEnabled();
});
