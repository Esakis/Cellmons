import { createHmac } from 'node:crypto';
import { expect, type Page } from '@playwright/test';
import { accessible } from './accessibility';

function otp(secret: string) {
  const alphabet = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ234567';
  let bits = '';
  for (const char of secret.replace(/=+$/, ''))
    bits += alphabet.indexOf(char).toString(2).padStart(5, '0');
  const bytes = Buffer.from((bits.match(/.{8}/g) ?? []).map((b) => Number.parseInt(b, 2)));
  const counter = Buffer.alloc(8);
  counter.writeBigUInt64BE(BigInt(Math.floor(Date.now() / 30000)));
  const hash = createHmac('sha1', bytes).update(counter).digest();
  const offset = (hash[19] ?? 0) & 15;
  return String((hash.readUInt32BE(offset) & 0x7fffffff) % 1000000).padStart(6, '0');
}
export async function administration(
  a: Page,
  b: Page,
  alice: string,
  bob: string,
  project: string,
) {
  a.setDefaultTimeout(10000);
  b.setDefaultTimeout(10000);
  for (const page of [a, b]) {
    await page.getByRole('button', { name: 'Map', exact: true }).click();
    await page.getByRole('button', { name: 'Arena', exact: true }).click();
    await expect(page.getByRole('heading', { name: 'Arena', exact: true })).toBeVisible();
  }
  const evidence = `Reportable message ${project}`;
  await b.getByRole('textbox', { name: 'Message', exact: true }).fill(evidence);
  await b.getByRole('button', { name: 'Send', exact: true }).click();
  await expect(a.getByRole('log')).toContainText(evidence);
  await a
    .getByRole('log')
    .getByRole('button', { name: `Report message from ${bob}`, exact: true })
    .last()
    .click();
  await a
    .getByRole('dialog')
    .getByLabel('Report reason')
    .fill('Moderation workflow acceptance evidence');
  await a.getByRole('dialog').getByRole('button', { name: 'Send report', exact: true }).click();
  await expect(a.getByRole('dialog')).toHaveCount(0);
  expect(
    (await a.request.post('http://127.0.0.1:18092/__make_staff', { data: { login: alice } })).ok(),
  ).toBe(true);
  await a.locator('header nav').getByRole('link', { name: 'Account', exact: true }).click();
  await a.getByRole('link', { name: 'Administration', exact: true }).click();
  await expect(a.getByRole('heading', { name: 'Administration', exact: true })).toBeVisible();
  await a.getByLabel('Password', { exact: true }).fill('Lobby integration password!');
  const setupReply = a.waitForResponse(
    (r) => r.url().endsWith('/api/admin/session') && r.request().method() === 'POST',
  );
  await a.getByRole('button', { name: 'Set up second factor', exact: true }).click();
  const setup = (await (await setupReply).json()) as { setup: { secret: string } };
  await a.getByLabel('Six-digit code', { exact: true }).fill(otp(setup.setup.secret));
  await a.getByRole('button', { name: 'Confirm code', exact: true }).click();
  await expect(a.getByRole('button', { name: 'Lock panel', exact: true })).toBeVisible();
  // The setup secret and credentials must disappear after confirmation.
  await expect(a.locator('.admin-secret')).toHaveCount(0);
  await expect(a.locator('input[type=password]')).toHaveCount(0);
  await a.getByRole('button', { name: 'Reports', exact: true }).click();
  const report = a.locator('.admin-rows li').filter({ hasText: evidence });
  await expect(report).toBeVisible();
  await report.getByRole('button', { name: 'Select', exact: true }).click();
  const reason = a.getByLabel('Decision reason (at least 5 characters)');
  await reason.fill('Reviewed report context in browser acceptance test');
  await a.getByRole('button', { name: 'Close as reviewed', exact: true }).click();
  await a.getByRole('dialog').getByRole('button', { name: 'Apply decision', exact: true }).click();
  await expect(a.locator('.admin-page').getByRole('status')).toHaveText('Decision saved.');
  await a.getByRole('button', { name: 'Players', exact: true }).click();
  await a.getByLabel('Search (reports: open, reviewed or rejected)').fill(bob);
  await a.locator('.admin-search').getByRole('button', { name: 'Reload', exact: true }).click();
  await a
    .locator('.admin-rows li')
    .filter({ hasText: bob })
    .getByRole('button', { name: 'Select', exact: true })
    .click();
  await a
    .locator('fieldset')
    .getByRole('combobox', { name: 'Action', exact: true })
    .selectOption('mute');
  await reason.fill('Temporary mute from browser acceptance test');
  await a.locator('fieldset').getByRole('button', { name: 'Review decision', exact: true }).click();
  let lost = false;
  const keys: string[] = [];
  await a.route('**/api/admin', async (route) => {
    const body = route.request().postDataJSON() as {
      request?: string;
      operation?: { action?: string };
    } | null;
    if (body?.operation?.action === 'mute') {
      keys.push(body.request ?? '');
      if (!lost) {
        lost = true;
        await route.fetch();
        await route.abort('failed');
        return;
      }
    }
    await route.continue();
  });
  await a.getByRole('dialog').getByRole('button', { name: 'Apply decision', exact: true }).click();
  await expect(a.getByRole('alert')).toContainText('No server response.');
  await a.getByRole('dialog').getByRole('button', { name: 'Apply decision', exact: true }).click();
  await expect(a.getByRole('dialog')).toHaveCount(0);
  expect(lost).toBe(true);
  expect(keys).toHaveLength(2);
  expect(new Set(keys).size).toBe(1);
  await a.unroute('**/api/admin');
  await b.getByRole('textbox', { name: 'Message', exact: true }).fill(`Blocked message ${project}`);
  await b.getByRole('button', { name: 'Send', exact: true }).click();
  await expect(b.getByRole('alert')).toContainText('Your account is muted.');
  await a
    .locator('.admin-rows li')
    .filter({ hasText: bob })
    .getByRole('button', { name: 'Select', exact: true })
    .click();
  await a
    .locator('fieldset')
    .getByRole('combobox', { name: 'Action', exact: true })
    .selectOption('unmute');
  await reason.fill('End of browser mute test');
  await a.locator('fieldset').getByRole('button', { name: 'Review decision', exact: true }).click();
  await a.getByRole('dialog').getByRole('button', { name: 'Apply decision', exact: true }).click();
  await expect(a.getByRole('dialog')).toHaveCount(0);
  await a
    .locator('.admin-rows li')
    .filter({ hasText: bob })
    .getByRole('button', { name: 'Select', exact: true })
    .click();
  await a
    .locator('fieldset')
    .getByRole('combobox', { name: 'Action', exact: true })
    .selectOption('role');
  await a.getByRole('combobox', { name: 'Role', exact: true }).selectOption('mentor');
  await reason.fill('Mentor badge acceptance test');
  await a.locator('fieldset').getByRole('button', { name: 'Review decision', exact: true }).click();
  await a.getByRole('dialog').getByRole('button', { name: 'Apply decision', exact: true }).click();
  await expect(a.getByRole('dialog')).toHaveCount(0);
  await b.locator('header nav').getByRole('link', { name: 'Account', exact: true }).click();
  await b.getByText(/^Moderation decisions \(\d+\)$/).click();
  await expect(b.locator('.account-notices')).toContainText(
    'Temporary mute from browser acceptance test',
  );
  await b.locator('header nav').getByRole('link', { name: 'Lobby', exact: true }).click();
  await expect(b.getByTestId('connection-status')).toHaveText('Connected');
  await b.getByRole('button', { name: 'Arena', exact: true }).click();
  await expect(b.locator('.lobby-player-list')).toContainText(`${bob} · Mentor`);
  await a.getByRole('button', { name: 'Audit', exact: true }).click();
  await expect(a.locator('.admin-rows')).toContainText(
    'Temporary mute from browser acceptance test',
  );
  await accessible(a);
  await a.screenshot({ path: `../target/admin-${project}.png`, fullPage: true });
  expect(
    await a.evaluate(
      () => document.documentElement.scrollWidth > document.documentElement.clientWidth,
    ),
  ).toBe(false);
  await a.getByRole('button', { name: 'Lock panel', exact: true }).click();
  await expect(a.getByRole('button', { name: 'Unlock panel', exact: true })).toBeVisible();
  await expect(a.locator('.admin-rows')).toHaveCount(0);
}
