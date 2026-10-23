import { expect, type Page } from '@playwright/test';
import { accessible } from './accessibility';
import { chooseLanguage } from './language';

export async function advancement(page: Page, login: string, project: string) {
  const panel = page.locator('.advancement');
  await panel.locator('summary').click();
  await expect(panel).toContainText('Card mastery');
  const priest = panel
    .locator('article')
    .filter({ has: page.getByRole('heading', { name: 'Priest', exact: true }) });
  await priest.getByRole('button', { name: 'Choose contract', exact: true }).click();
  await expect(priest.getByRole('button', { name: 'Active contract', exact: true })).toBeDisabled();
  await page.reload();
  await panel.locator('summary').click();
  await expect(priest).toContainText('Active contract');
  expect(
    (
      await page.request.post('http://127.0.0.1:18092/__fragments', {
        data: { login, template: 2 },
      })
    ).ok(),
  ).toBe(true);
  await page.reload();
  await panel.locator('summary').click();
  await expect(priest).toContainText('30/30');
  const before = await (await page.request.get('/api/game')).json();
  await priest.getByRole('button', { name: 'Claim character', exact: true }).click();
  await expect(priest).toHaveCount(0);
  const after = await (await page.request.get('/api/game')).json();
  expect(after.avatars).toHaveLength(before.avatars.length + 1);
  expect(after.recruitment.milestones).toEqual(before.recruitment.milestones);
  expect(after.avatars.find((a: { template_id: number }) => a.template_id === 2).deck).toHaveLength(
    20,
  );
  // A tile shows mastery only above the starting 1/5; the card preview always does.
  await page.locator('.deck-library .deck-preview').first().click();
  await expect(page.getByRole('dialog')).toContainText('Mastery 1/5');
  await page.getByRole('dialog').getByRole('button', { name: 'Close', exact: true }).click();
  if (project.startsWith('mobile')) {
    await page.setViewportSize({ width: 320, height: 640 });
    await chooseLanguage(page, 'pl');
    await expect(panel).toContainText('Opanowanie kart');
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(
      true,
    );
    await panel.screenshot({ path: '../target/advancement-pl-320.png' });
    await chooseLanguage(page, 'en');
    await page.setViewportSize({ width: 393, height: 727 });
  }
  await accessible(page);
}
