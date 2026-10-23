import { expect, type Page } from '@playwright/test';
import { accessible } from './accessibility';

// Reuses an existing real account so the full suite keeps production registration limits.
export async function progression(page: Page, login: string, project: string) {
  await page.locator('header nav').getByRole('link', { name: 'Collection', exact: true }).click();
  const state = (await (await page.request.get('/api/game')).json()) as {
    avatars: { id: string; active: boolean; deck: number[] }[];
  };
  const source = state.avatars.find((a) => a.active);
  expect(source).toBeTruthy();
  const id = source?.id ?? '';
  for (const stage of ['path', 'specialization']) {
    expect(
      (
        await page.request.post('http://127.0.0.1:18092/__advance', { data: { login, avatar: id } })
      ).ok(),
    ).toBe(true);
    await page.reload();
    const choose = page.getByRole('button', { name: new RegExp(`^Choose ${stage}:`) }).first();
    await expect(choose).toBeEnabled();
    await choose.click();
    const dialog = page.getByRole('dialog');
    await expect(dialog).toContainText('This choice is permanent');
    await dialog.getByRole('button', { name: 'Confirm change', exact: true }).click();
    await expect(page.getByRole('status').filter({ hasText: 'Changes saved.' })).toBeVisible();
    const data = await (await page.request.get('/api/game')).json();
    const current = data.avatars.find((a: { id: string }) => a.id === id);
    expect(current.level).toBe(1);
    expect(current.xp).toBe(0);
    expect(current.deck).toHaveLength(20);
    expect(current.archived_decks[0]).toEqual(source?.deck);
    expect(current.advancement.permanent_points).toBe(stage === 'path' ? 3 : 6);
  }
  await page
    .locator('.collection-progression')
    .screenshot({ path: `../target/reincarnation-${project}.png` });
  // Test the actual path/gift in local WASM before checking the retired shade gate.
  await page.getByRole('link', { name: 'Test deck offline', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Confirm move', exact: true })).toBeEnabled();
  // The app uses its own forfeit modal when leaving a local duel.
  await page.locator('header nav').getByRole('link', { name: 'Collection', exact: true }).click();
  await page.getByRole('dialog').getByRole('button', { name: 'Forfeit duel', exact: true }).click();
  await expect(
    page.getByRole('heading', { name: 'Characters and collection', exact: true }),
  ).toBeVisible();
  expect(
    (
      await page.request.post('http://127.0.0.1:18092/__advance', { data: { login, avatar: id } })
    ).ok(),
  ).toBe(true);
  await page.reload();
  await expect(page.getByRole('button', { name: 'Sacrifice for shade', exact: true })).toHaveCount(
    0,
  );
  const before = await (await page.request.get('/api/game')).json();
  const current = before.avatars.find((a: { id: string }) => a.id === id);
  const denied = await page.request.post('/api/game', {
    headers: { Origin: new URL(page.url()).origin, 'X-Requested-With': 'app' },
    data: {
      action: 'shade',
      id,
      revision: current.revision,
      new_id: crypto.randomUUID(),
      school: 'arcane',
    },
  });
  expect((await denied.json()).error).toBe('AVATAR_LOCKED');
  const after = await (await page.request.get('/api/game')).json();
  expect(after.avatars).toEqual(before.avatars);
  await accessible(page);
}
