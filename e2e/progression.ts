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
  for (const stage of ['path', 'specialization', 'gift']) {
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
    expect(current.deck).toEqual(source?.deck);
  }
  await page
    .locator('.collection-progression')
    .screenshot({ path: `../target/reincarnation-${project}.png` });
  // Test the actual path/gift in local WASM before sacrificing the character.
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
  await page
    .getByRole('combobox', { name: 'Shade school', exact: true })
    .selectOption('arcane', { timeout: 10000 });
  await page.getByRole('button', { name: 'Sacrifice for shade', exact: true }).click();
  await expect(page.getByRole('dialog')).toContainText('permanently consumed');
  await page
    .getByRole('dialog')
    .getByRole('button', { name: 'Confirm change', exact: true })
    .click();
  await expect(page.getByRole('heading', { name: 'Shade of Time', exact: true })).toBeVisible();
  const after = await (await page.request.get('/api/game')).json();
  expect(after.avatars.some((a: { id: string }) => a.id === id)).toBe(false);
  expect(after.avatars.find((a: { active: boolean }) => a.active).shade_school).toBe('arcane');
  await expect(page.getByRole('button', { name: /^Choose path:/ })).toHaveCount(2);
  await expect(page.getByRole('button', { name: /^Choose path:/ }).first()).toBeDisabled();
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth > document.documentElement.clientWidth,
    ),
  ).toBe(false);
  await page
    .locator('.collection-progression')
    .screenshot({ path: `../target/shade-${project}.png` });
  await accessible(page);
}
