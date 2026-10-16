import { readFile } from 'node:fs/promises';
import { expect, type Page } from '@playwright/test';
import { accessible } from './accessibility';

export async function privacy(page: Page, login: string, project: string) {
  await page.locator('header nav').getByRole('link', { name: 'Account', exact: true }).click();
  await page.getByText('Data and account deletion', { exact: true }).click();
  const controls = page.locator('.privacy-controls');
  await expect(controls).toContainText(`Account: ${login}`);
  await accessible(page);
  const downloading = page.waitForEvent('download');
  await controls.getByRole('button', { name: 'Download account data (JSON)', exact: true }).click();
  const download = await downloading;
  expect(download.suggestedFilename()).toBe('spellgambit-data.json');
  const path = await download.path();
  expect(path).toBeTruthy();
  const data = JSON.parse(await readFile(path as string, 'utf8')) as {
    account: { login: string }[];
  };
  expect(data.account[0]?.login).toBe(login);
  expect(JSON.stringify(data)).not.toMatch(/password_hash|id_hash|token_hash/);
  await controls.getByRole('button', { name: 'Delete account', exact: true }).click();
  const modal = page.getByRole('dialog');
  const confirm = modal.getByRole('button', { name: 'Request account deletion', exact: true });
  await expect(confirm).toBeDisabled();
  await accessible(page);
  await modal.getByLabel('Enter the exact account login').fill(login);
  await modal.getByLabel('Current password').fill('Lobby integration password!');
  await modal.getByRole('checkbox').check();
  await modal.getByLabel('Current password').fill('An incorrect fixture password!');
  await confirm.click();
  await expect(controls.getByRole('alert')).toContainText('The password is incorrect.');
  await expect(modal).toBeVisible();
  await modal.getByLabel('Current password').fill('Lobby integration password!');
  let lost = false;
  await page.route('**/api/me', async (route) => {
    if (route.request().method() === 'DELETE') {
      lost = true;
      const response = await route.fetch();
      expect(response.status()).toBe(204);
      await route.abort('failed');
    } else await route.continue();
  });
  await confirm.click();
  await expect(controls.getByRole('status')).toContainText('Request accepted and sessions ended.');
  expect(lost).toBe(true);
  await page.unroute('**/api/me');
  await expect(page.getByRole('heading', { name: login, exact: true })).toHaveCount(0);
  await expect.poll(async () => (await page.request.get('/api/me')).status()).toBe(401);
  const restore = page.getByRole('form', { name: 'Cancel a deletion request', exact: true });
  await restore.getByLabel('Login or email of the account to restore').fill(login);
  await restore.getByLabel('Current password').fill('Lobby integration password!');
  await restore.getByRole('button', { name: 'Cancel account deletion', exact: true }).click();
  await expect(controls.getByRole('status')).toContainText('Request cancelled.');
  await expect(restore.getByLabel('Current password')).toHaveValue('');
  await expect(page.getByRole('heading', { name: login, exact: true })).toHaveCount(0);
  await page.screenshot({ path: `../target/privacy-${project}.png`, fullPage: true });
  await accessible(page);
}
