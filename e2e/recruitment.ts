import { expect, type Page } from '@playwright/test';
import { accessible } from './accessibility';

export async function recruitment(page: Page, project: string) {
  await page.locator('header nav').getByRole('link', { name: 'Collection', exact: true }).click();
  const panel = page.locator('.recruitment');
  await expect(panel).toContainText('Ready to claim: 7');
  await expect(panel.locator('.recruit-milestones li')).toHaveCount(7);
  await expect(
    panel.getByRole('button', { name: 'Meet character: Warrior', exact: true }),
  ).toHaveCount(0);
  const before = await (await page.request.get('/api/game')).json();
  const active = before.avatars.find((a: { active: boolean }) => a.active).id;
  await panel.getByRole('button', { name: 'Meet character: Mage', exact: true }).click();
  const dialog = page.getByRole('dialog');
  await expect(dialog).toContainText('focus costs');
  await dialog.locator('summary').click();
  await expect(dialog.locator('.recruit-deck li')).toHaveCount(6);
  await expect(dialog.locator('.recruit-deck img').first()).toBeVisible();
  await accessible(page);
  await dialog.screenshot({ path: `../target/recruitment-preview-${project}.png` });
  await dialog.getByRole('button', { name: 'Recruit: Mage', exact: true }).click();
  await expect(dialog).toHaveCount(0);
  await expect(panel).toContainText('Ready to claim: 6');
  await expect(
    panel.getByRole('button', { name: 'Meet character: Mage', exact: true }),
  ).toHaveCount(0);
  await expect(page.getByRole('status').filter({ hasText: 'Reward claimed.' })).toBeVisible();
  const after = await (await page.request.get('/api/game')).json();
  expect(after.avatars.find((a: { active: boolean }) => a.active).id).toBe(active);
  const mage = after.avatars.find((a: { template_id: number }) => a.template_id === 4);
  expect(mage.deck).toHaveLength(20);
  expect(mage.level).toBe(1);
  expect(after.recruitment.milestones[0].avatar_id).toBe(mage.id);
  // A lost response may cause a retry. The real endpoint must return the same award.
  const again = await page.request.post('/api/game', {
    headers: { Origin: new URL(page.url()).origin, 'X-Requested-With': 'app' },
    data: { action: 'recruit', level: 5, template_id: 4 },
  });
  expect(again.ok()).toBe(true);
  const retried = await again.json();
  expect(retried.avatars).toEqual(after.avatars);
  expect(retried.collection).toEqual(after.collection);
  await page.reload();
  await expect(panel).toContainText('Ready to claim: 6');
  await expect(panel.locator('.recruit-milestones li').first()).toContainText('Mage');
  if (project.startsWith('mobile')) {
    await page.setViewportSize({ width: 320, height: 640 });
    await page.locator('header select').selectOption('pl');
    await expect(panel).toContainText('Do odebrania: 6');
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(
      true,
    );
    await panel.screenshot({ path: '../target/recruitment-pl-320.png' });
    await page.locator('header select').selectOption('en');
    await page.setViewportSize({ width: 393, height: 727 });
  }
  await accessible(page);
}
