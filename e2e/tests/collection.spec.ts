import { expect, test } from '@playwright/test';

test('guest builds a RAM-only collection and tests its deck offline', async ({
  page,
  context,
}, info) => {
  const network: string[] = [];
  page.on('request', (request) => {
    if (/\/api\/|\/ws(?:\?|$)/.test(request.url())) network.push(request.url());
  });
  await page.addInitScript(() => {
    localStorage.setItem('game.language', 'en');
    localStorage.setItem('game.speed', 'instant');
  });
  await page.goto('/');
  await page.getByRole('button', { name: 'Play as guest', exact: true }).click();
  await page.locator('header nav').getByRole('link', { name: 'Collection', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Characters and collection' })).toBeVisible();
  await expect(page.locator('.collection-avatars button')).toHaveCount(1);
  await page.getByLabel('School requirements met', { exact: true }).uncheck();
  await expect(page.locator('.collection-cards article')).toHaveCount(24);
  await page.getByRole('button', { name: 'Show more spells', exact: true }).click();
  await expect(page.locator('.collection-cards article')).toHaveCount(48);
  await page.getByRole('searchbox', { name: 'Find spell', exact: true }).fill('Primal Heart');
  await expect(page.locator('.collection-cards article')).toHaveCount(1);
  await expect(
    page.getByRole('button', { name: 'Preview card: Primal Heart', exact: true }),
  ).toBeVisible();
  await page.getByRole('searchbox', { name: 'Find spell', exact: true }).fill('');
  await expect(page.locator('.collection-cards article')).toHaveCount(24);
  await page.getByRole('button', { name: 'Create character', exact: true }).click();
  await expect(page.locator('.collection-avatars button')).toHaveCount(2);
  await page.getByRole('button', { name: 'Activate', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Deck (20/20)', exact: true })).toBeVisible();
  const library = page.locator('.deck-library');
  await library.getByRole('button', { name: 'Preview card: Bronze Blade', exact: true }).click();
  await expect(page.getByRole('dialog').locator('img.deck-detail-art')).toBeVisible();
  await page.getByRole('dialog').getByRole('button', { name: 'Close', exact: true }).click();
  await page.getByRole('button', { name: 'Sample hand', exact: true }).click();
  await expect(page.getByRole('dialog').locator('.deck-sample img')).toHaveCount(5);
  await page.getByRole('dialog').getByRole('button', { name: 'Close', exact: true }).click();
  await library
    .getByRole('button', { name: 'Remove from deck: Bronze Blade', exact: true })
    .click();
  await expect(page.getByRole('heading', { name: 'Deck (19/20)', exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Undo change', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Deck (20/20)', exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Clear', exact: true }).click();
  for (let i = 0; i < 4; i++)
    await page
      .locator('.deck-library')
      .getByRole('button', { name: 'Add to deck: Bronze Blade', exact: true })
      .click();
  await expect(
    page
      .locator('.deck-library')
      .getByRole('button', { name: 'Add to deck: Bronze Blade', exact: true }),
  ).toBeDisabled();
  await page.getByRole('button', { name: 'Save deck', exact: true }).click();
  await expect(page.getByRole('status').filter({ hasText: 'Changes saved.' })).toBeVisible();
  await page.locator('.collection-deck-heading').scrollIntoViewIfNeeded();
  await page.screenshot({ path: `../target/deck-editor-${info.project.name}.png` });
  await page.screenshot({ path: `../target/collection-${info.project.name}.png`, fullPage: true });
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth > document.documentElement.clientWidth,
    ),
  ).toBe(false);
  expect(
    await page.evaluate(() =>
      Object.values(localStorage).some((v) => /template_id|deck_revision|"card_id"/.test(v)),
    ),
  ).toBe(false);
  await page.locator('header nav').getByRole('link', { name: 'Store', exact: true }).click();
  await expect(
    page.getByText('The store requires an account and an internet connection.'),
  ).toBeVisible();
  await expect(page.getByRole('link', { name: 'Account', exact: true }).last()).toBeVisible();
  await expect(page.locator('.store-page')).toHaveCount(0);
  await page.evaluate(() => {
    location.hash = '#/clans';
  });
  await expect(page.getByRole('heading', { name: 'Clans', exact: true })).toBeVisible();
  await expect(
    page.getByText('Clans are available to registered accounts while connected to the server.'),
  ).toBeVisible();
  await expect(page.locator('.clans-page')).toHaveCount(0);
  await page.locator('header nav').getByRole('link', { name: 'Collection', exact: true }).click();
  await expect(page.getByTestId('offline-status')).toHaveText('Ready to play offline');
  await context.setOffline(true);
  await page.getByRole('link', { name: 'Test deck offline', exact: true }).click();
  const confirm = page.getByRole('button', { name: 'Confirm move', exact: true });
  for (let round = 0; round < 5; round++) {
    if (await page.locator('#duel-result').count()) break;
    await expect(confirm).toBeEnabled();
    await confirm.click();
    await expect(
      confirm.or(page.getByRole('button', { name: 'Play again', exact: true })),
    ).toBeEnabled();
  }
  await expect(page.locator('#duel-result')).toBeVisible();
  await page.locator('header nav').getByRole('link', { name: 'Collection', exact: true }).click();
  await expect(page.locator('.collection-avatars button')).toHaveCount(2);
  await page.reload();
  await page.getByRole('button', { name: 'Guest', exact: true }).click();
  await expect(page.locator('.collection-avatars button')).toHaveCount(1);
  expect(network).toEqual([]);
});
