import { expect, type Page } from '@playwright/test';
import { accessible } from './accessibility';

export async function expeditions(a: Page, b: Page, project: string) {
  for (const page of [a, b]) {
    // A building has only the way back to the map; the portal is entered from there.
    await page.getByRole('button', { name: 'Map', exact: true }).click();
    await page.getByRole('button', { name: 'Portal', exact: true }).click();
    await expect(page.getByTestId('expeditions')).toBeVisible();
  }
  const first = a.getByTestId('expeditions');
  const second = b.getByTestId('expeditions');
  const keys: string[] = [];
  let drop = true;
  await a.route('**/api/expeditions', async (route) => {
    const request = route.request();
    if (request.method() === 'POST' && request.postDataJSON()?.action === 'start') {
      keys.push(request.postDataJSON().id);
      if (drop) {
        drop = false;
        await route.fetch();
        await route.abort('failed');
        return;
      }
    }
    await route.continue();
  });
  await first.getByRole('button', { name: 'Start expedition', exact: true }).click();
  await expect(first.getByRole('alert')).toContainText('did not confirm');
  await first.getByRole('button', { name: 'Retry the same request', exact: true }).click();
  await expect(first.locator('.exp-enemy')).toHaveCount(3);
  expect(keys).toHaveLength(2);
  expect(keys[0]).toBe(keys[1]);
  await a.unroute('**/api/expeditions');
  await first.getByRole('button', { name: "Expedition participants' chat", exact: true }).click();
  const secret = `Expedition-only ${project}`;
  await a.getByRole('textbox', { name: 'Message', exact: true }).fill(secret);
  await a.getByRole('button', { name: 'Send', exact: true }).click();
  await expect(a.getByRole('log')).toContainText(secret);
  await expect(b.getByRole('log')).not.toContainText(secret);
  await second.getByRole('button', { name: 'Start expedition', exact: true }).click();
  await second.getByRole('button', { name: "Expedition participants' chat", exact: true }).click();
  await expect(b.getByRole('log')).toContainText(secret);
  await second
    .getByRole('button', { name: /^Fight:/ })
    .first()
    .click();
  const duel = b.getByTestId('online-duel');
  await expect(duel).toBeVisible({ timeout: 10000 });
  await duel.getByRole('button', { name: 'Surrender', exact: true }).click();
  await b.getByRole('dialog').getByRole('button', { name: 'Surrender', exact: true }).click();
  await expect(duel.getByRole('button', { name: 'Close', exact: true })).toBeVisible();
  await duel.getByRole('button', { name: 'Close', exact: true }).click();
  await expect(b.getByTestId('expeditions').getByRole('status')).toContainText('XP received: 0');
  await expect(b.getByRole('heading', { name: 'Portal', exact: true })).toBeVisible();
  const after = `After the expedition ${project}`;
  await a.getByRole('textbox', { name: 'Message', exact: true }).fill(after);
  await a.getByRole('button', { name: 'Send', exact: true }).click();
  await expect(a.getByRole('log')).toContainText(after);
  await expect(b.getByRole('log')).not.toContainText(after);
  await first.getByRole('button', { name: 'Withdraw from expedition', exact: true }).click();
  await a.getByRole('dialog').getByRole('button', { name: 'Close', exact: true }).click();
  await expect(first.locator('.exp-enemy')).toHaveCount(3);
  await first.getByRole('button', { name: 'Withdraw from expedition', exact: true }).click();
  await a
    .getByRole('dialog')
    .getByRole('button', { name: 'Confirm withdrawal', exact: true })
    .click();
  await expect(first.getByRole('status')).toContainText('XP received: 0');
  await first.screenshot({ path: `../target/expeditions-${project}.png` });
  expect(
    await a.evaluate(
      () => document.documentElement.scrollWidth > document.documentElement.clientWidth,
    ),
  ).toBe(false);
  await accessible(a);
}
