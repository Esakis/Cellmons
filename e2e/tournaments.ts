import { expect, type Page } from '@playwright/test';
import { accessible } from './accessibility';

export async function tournaments(a: Page, b: Page, alice: string, project: string) {
  for (const page of [a, b]) {
    await page.goto('/#/lobby');
    await expect(page.getByTestId('connection-status')).toHaveText('Connected');
    await page.getByRole('button', { name: 'Castle', exact: true }).click();
    await expect(page.getByTestId('tournaments')).toBeVisible();
  }
  const first = a.getByTestId('tournaments');
  const second = b.getByTestId('tournaments');
  await first.getByRole('button', { name: 'Create a tournament', exact: true }).click();
  const name = `Cup ${alice}`;
  await first.getByLabel('Tournament name', { exact: true }).fill(name);
  await first.getByLabel('Number of players', { exact: true }).fill('2');
  await first.getByLabel('Maximum fights per phase', { exact: true }).fill('1');
  const keys: string[] = [];
  let drop = true;
  await a.route('**/api/tournaments', async (route) => {
    const request = route.request();
    if (request.method() === 'POST' && request.postDataJSON()?.action === 'create') {
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
  await first.getByRole('button', { name: 'Publish tournament', exact: true }).click();
  await expect(first.getByRole('alert')).toContainText('did not confirm');
  await first.getByRole('button', { name: 'Retry the same request', exact: true }).click();
  await expect(first.getByRole('heading', { name, exact: true })).toBeVisible();
  expect(keys).toHaveLength(2);
  expect(keys[0]).toBe(keys[1]);
  await a.unroute('**/api/tournaments');
  await first.getByRole('button', { name: 'Join for 0 D', exact: true }).click();
  await expect(first).toContainText('You are enrolled.');
  await first.getByRole('button', { name: 'Leave tournament', exact: true }).click();
  await expect(a.getByRole('dialog')).toContainText('full refund');
  await a.getByRole('dialog').getByRole('button', { name: 'Confirm', exact: true }).click();
  await expect(first.getByRole('button', { name: 'Join for 0 D', exact: true })).toBeVisible();
  await first.getByRole('button', { name: 'Join for 0 D', exact: true }).click();
  await second.getByRole('button', { name: 'Reload', exact: true }).click();
  await second.getByRole('button', { name, exact: true }).click();
  await second.getByRole('button', { name: 'Join for 0 D', exact: true }).click();
  const duelA = a.getByTestId('online-duel');
  const duelB = b.getByTestId('online-duel');
  await expect(duelA).toBeVisible({ timeout: 35000 });
  await expect(duelB).toBeVisible({ timeout: 10000 });
  await duelB.getByRole('button', { name: 'Surrender', exact: true }).click();
  await b.getByRole('dialog').getByRole('button', { name: 'Surrender', exact: true }).click();
  await expect(duelA).toContainText('Victory');
  await duelA.getByRole('button', { name: 'Close', exact: true }).click();
  await duelB.getByRole('button', { name: 'Close', exact: true }).click();
  await expect(a.getByTestId('tournaments').getByRole('status')).toContainText(
    `Champion: ${alice}`,
    { timeout: 10000 },
  );
  await a.getByTestId('tournaments').screenshot({ path: `../target/tournaments-${project}.png` });
  expect(
    await a.evaluate(
      () => document.documentElement.scrollWidth > document.documentElement.clientWidth,
    ),
  ).toBe(false);
  // Restore the last tournament after a full reload, without rejoining or charging again.
  await a.reload();
  await expect(a.getByTestId('connection-status')).toHaveText('Connected');
  await a.getByRole('button', { name: 'Castle', exact: true }).click();
  await a.getByTestId('tournaments').getByRole('button', { name, exact: true }).click();
  await expect(a.getByTestId('tournaments')).toContainText(`Champion: ${alice}`);
  await accessible(a);
}
