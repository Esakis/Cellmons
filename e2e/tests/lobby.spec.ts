import { expect, type Page, test } from '@playwright/test';
import { administration } from '../admin';
import { clans } from '../clans';
import { expeditions } from '../expeditions';
import { privacy } from '../privacy';
import { progression } from '../progression';
import { tournaments } from '../tournaments';

// Real registrations deliberately keep the production IP limit; retries need a fresh fixture DB.
test.describe.configure({ retries: 0 });

async function register(page: Page, login: string, build = false) {
  await page.addInitScript(() => localStorage.setItem('game.language', 'en'));
  await page.goto('/#/account');
  await page.getByRole('button', { name: 'Register', exact: true }).click();
  await page.getByLabel('Login (3–20 letters, digits, _ or -)').fill(login);
  await page.getByLabel('Email', { exact: true }).fill(`${login}@example.test`);
  await page.getByLabel('Password', { exact: true }).fill('Lobby integration password!');
  await page.locator('input[name=agreement]').check();
  await page.getByRole('button', { name: 'Create account', exact: true }).click();
  await expect(page.getByRole('status').filter({ hasText: 'Account created.' })).toBeVisible();
  // Read the captured email from the loopback-only disposable backend, then use the real verification UI.
  const mailbox = `http://127.0.0.1:18092/__mail?to=${encodeURIComponent(`${login}@example.test`)}`;
  await expect
    .poll(async () => ((await (await page.request.get(mailbox)).json()) as unknown[]).length)
    .toBeGreaterThan(0);
  const deliveries = (await (await page.request.get(mailbox)).json()) as { body: string }[];
  const link = deliveries[0]?.body.match(/https?:\/\/\S+/)?.[0];
  expect(link).toBeTruthy();
  await page.goto(link as string);
  await page.getByRole('button', { name: 'Verify email', exact: true }).click();
  await expect(
    page.getByRole('status').filter({ hasText: 'Email address verified.' }),
  ).toBeVisible();
  await page.getByLabel('Login or email').fill(login);
  await page.getByLabel('Password', { exact: true }).fill('Lobby integration password!');
  await page.getByRole('button', { name: 'Sign in', exact: true }).last().click();
  await expect(page.getByRole('heading', { name: login })).toBeVisible();
  if (build) {
    await page.locator('header nav').getByRole('link', { name: 'Collection', exact: true }).click();
    await expect(page.getByRole('heading', { name: 'Characters and collection' })).toBeVisible();
    await page.getByRole('button', { name: 'Create character', exact: true }).click();
    await expect(page.locator('.collection-avatars button')).toHaveCount(2);
    await page.getByRole('button', { name: 'Activate', exact: true }).click();
    await page.getByRole('button', { name: 'Add to deck: Bronze Blade', exact: true }).click();
    await page.getByRole('button', { name: 'Save deck', exact: true }).click();
    await expect(page.getByRole('status').filter({ hasText: 'Changes saved.' })).toBeVisible();
    await page.getByLabel('Preset name').fill('E2E deck');
    await page.getByRole('button', { name: 'Save preset', exact: true }).click();
    await expect(page.getByRole('button', { name: 'E2E deck', exact: true })).toBeVisible();
  }
  await page.locator('header nav').getByRole('link', { name: 'Lobby', exact: true }).click();
  await expect(page.getByTestId('connection-status')).toHaveText('Connected');
  await page.getByRole('button', { name: 'Arena', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Arena', exact: true })).toBeVisible();
}

test('real registration, two-client chat, contacts, mail, challenges and reconnect', async ({
  browser,
}, info) => {
  test.setTimeout(180000);
  const first = await browser.newContext(info.project.use);
  const second = await browser.newContext(info.project.use);
  try {
    const a = await first.newPage();
    const b = await second.newPage();
    const suffix = `${info.project.name.startsWith('mobile') ? 'm' : 'd'}${Date.now().toString(36)}`;
    const alice = `Alice_${suffix}`;
    const bob = `Bob_${suffix}`;
    const greeting = `Hello <script>window.pwned=true</script> ${suffix}`;
    const reply = `Hello back from the second client ${suffix}`;
    await register(a, alice, true);
    await register(b, bob);
    await expect(a.getByRole('button').filter({ hasText: bob }).first()).toBeVisible();
    await a.getByRole('textbox', { name: 'Message', exact: true }).fill(greeting);
    await a.getByRole('button', { name: 'Send', exact: true }).click();
    await expect(b.getByRole('log')).toContainText(greeting);
    expect(await b.evaluate(() => 'pwned' in window)).toBe(false);
    await b.getByRole('textbox', { name: 'Message', exact: true }).fill(reply);
    await b.getByRole('button', { name: 'Send', exact: true }).click();
    await expect(a.getByRole('log')).toContainText(reply);
    // The recipient must add an unverified sender before private messages or mail are allowed.
    await b.getByRole('textbox', { name: 'Message', exact: true }).fill(`/f add ${alice}`);
    await b.getByRole('button', { name: 'Send', exact: true }).click();
    await expect(b.getByRole('textbox', { name: 'Message', exact: true })).toHaveValue('');
    await a.getByRole('button', { name: 'Mail', exact: true }).click();
    await a.getByLabel('Recipient (login)').fill(bob);
    await a.locator('textarea[name=text]').fill('A persistent offline letter');
    await a.getByRole('button', { name: 'Send mail', exact: true }).click();
    await expect(a.getByRole('status').filter({ hasText: 'Mail sent.' })).toBeVisible();
    await b.getByRole('button', { name: 'Mail', exact: true }).click();
    await expect(b.locator('.lobby-letter')).toContainText('A persistent offline letter');
    await b.getByRole('button', { name: 'Mark as read' }).click();
    await expect(b.getByRole('button', { name: 'Mark as read' })).toHaveCount(0);
    await a.getByRole('button', { name: 'Map', exact: true }).click();
    await a.locator('.lobby-player-list button').filter({ hasText: bob }).click();
    await a.getByRole('button', { name: 'Challenge', exact: true }).click();
    await expect(b.locator('.lobby-offer')).toContainText(alice);
    await b.getByRole('button', { name: 'Decline', exact: true }).click();
    await expect(a.locator('.lobby-offer')).toHaveCount(0);
    await first.setOffline(true);
    await expect(a.getByTestId('connection-status')).toHaveText('Offline');
    await first.setOffline(false);
    await expect(a.getByTestId('connection-status')).toHaveText('Connected');
    await expect(a.getByRole('heading', { name: 'Arena', exact: true })).toBeVisible();
    await a.reload();
    await expect(a.getByTestId('connection-status')).toHaveText('Connected');
    await a.getByRole('button', { name: 'Arena', exact: true }).click();
    await expect(a.getByRole('log')).toContainText(reply);
    await a.screenshot({ path: `../target/lobby-${info.project.name}.png`, fullPage: true });
    expect(
      await a.evaluate(
        () => document.documentElement.scrollWidth > document.documentElement.clientWidth,
      ),
    ).toBe(false);
    expect(
      await a.evaluate(() =>
        Object.values(localStorage).some((value) =>
          /Alice_|Bob_|persistent offline letter/.test(value),
        ),
      ),
    ).toBe(false);
    // The same authenticated clients complete a server-authoritative duel.
    await a.locator('.lobby-player-list button').filter({ hasText: bob }).click();
    await a.getByRole('button', { name: 'Challenge', exact: true }).click();
    await b.getByRole('button', { name: 'Accept', exact: true }).click();
    const duelA = a.getByTestId('online-duel');
    const duelB = b.getByTestId('online-duel');
    await expect(duelA).toBeVisible();
    await expect(duelB).toBeVisible();
    for (let round = 1; round <= 5; round++) {
      await expect(duelA.getByRole('heading').first()).toContainText(`Round ${round} / 5`);
      await expect(duelB.getByRole('heading').first()).toContainText(`Round ${round} / 5`);
      await duelA.getByRole('button', { name: 'Submit move', exact: true }).click();
      await expect(duelA).toContainText('Move saved. Waiting for your opponent.');
      if (round === 1) {
        await a.screenshot({
          path: `../target/online-duel-${info.project.name}.png`,
          fullPage: true,
        });
        expect(
          await a.evaluate(
            () => document.documentElement.scrollWidth > document.documentElement.clientWidth,
          ),
        ).toBe(false);
        await first.setOffline(true);
        await expect(a.getByTestId('connection-status')).toHaveText('Offline');
        await first.setOffline(false);
        await expect(a.getByTestId('connection-status')).toHaveText('Connected');
        await expect(duelA).toContainText('Move saved. Waiting for your opponent.');
      }
      if (round === 2) {
        await a.reload();
        await expect(a.getByTestId('connection-status')).toHaveText('Connected');
        await expect(duelA).toContainText('Move saved. Waiting for your opponent.');
      }
      await duelB.getByRole('button', { name: 'Submit move', exact: true }).click();
    }
    await expect(duelA.locator('.online-result')).toContainText('Draw');
    await expect(duelA.locator('.online-result')).toContainText('+50 PD');
    await expect(duelB.locator('.online-result')).toContainText('+50 PD');
    await duelA.getByRole('button', { name: 'Close', exact: true }).click();
    await a.getByRole('button', { name: 'Match history', exact: true }).click();
    await expect(a.locator('.online-match')).toContainText(alice);
    await a.getByRole('button', { name: 'Replay', exact: true }).click();
    await expect(a.getByLabel('Replay position')).toBeVisible();
    await a.getByRole('button', { name: 'Map', exact: true }).click();
    await a.getByRole('button', { name: 'Training', exact: true }).click();
    await expect(a.locator('.online-bots button')).toHaveCount(8);
    await a.locator('.online-bots button').first().click();
    await expect(duelA).toContainText('Opponent ready');
    await duelA.getByRole('button', { name: 'Surrender', exact: true }).click();
    await a.getByRole('dialog').getByRole('button', { name: 'Surrender', exact: true }).click();
    await expect(duelA.locator('.online-result')).toContainText('+0 PD');
    await duelA.getByRole('button', { name: 'Close', exact: true }).click();
    await duelB.getByRole('button', { name: 'Close', exact: true }).click();
    // A lost purchase reply must be retried with the original key.
    await a.locator('header nav').getByRole('link', { name: 'Store', exact: true }).click();
    await expect(a.getByTestId('wallet-balance')).toHaveText('Dukats: 40');
    await a.getByRole('button', { name: 'Contents and odds', exact: true }).first().click();
    await expect(a.getByRole('dialog')).toContainText('with replacement');
    await a.getByRole('dialog').getByRole('button', { name: 'Close', exact: true }).click();
    const purchaseKeys: string[] = [];
    await a.route('**/api/store', async (route) => {
      if (route.request().method() === 'POST') {
        const request = route.request().postDataJSON();
        if (request.operation.action === 'buy') {
          purchaseKeys.push(request.key);
          if (purchaseKeys.length === 1) {
            await route.fetch();
            await route.abort('failed');
            return;
          }
        }
      }
      await route.continue();
    });
    await a.getByRole('button', { name: 'Buy Basic pack', exact: true }).click();
    await a.getByRole('dialog').getByRole('button', { name: 'Confirm', exact: true }).click();
    await expect(a.getByRole('alert')).toContainText('did not confirm');
    await a.getByRole('button', { name: 'Retry operation', exact: true }).click();
    await expect(a.locator('.store-receipt')).toContainText('Operation completed');
    await expect(a.getByTestId('wallet-balance')).toHaveText('Dukats: 0');
    expect(purchaseKeys).toHaveLength(2);
    expect(purchaseKeys[0]).toBe(purchaseKeys[1]);
    await expect(a.getByRole('button', { name: 'Open pack', exact: true })).toHaveCount(1);
    await a.getByRole('button', { name: 'Open pack', exact: true }).click();
    await expect(a.locator('.store-receipt li')).toHaveCount(12);
    await expect(a.getByRole('button', { name: 'Open pack', exact: true })).toHaveCount(0);
    await a.screenshot({ path: `../target/store-${info.project.name}.png`, fullPage: true });
    expect(
      await a.evaluate(
        () => document.documentElement.scrollWidth > document.documentElement.clientWidth,
      ),
    ).toBe(false);
    await a.locator('header nav').getByRole('link', { name: 'Lobby', exact: true }).click();
    await expect(a.getByTestId('connection-status')).toHaveText('Connected');
    await a.getByRole('button', { name: 'Bazaar', exact: true }).click();
    await b.getByRole('button', { name: 'Map', exact: true }).click();
    await b.getByRole('button', { name: 'Bazaar', exact: true }).click();
    await a.locator('.lobby-player-list button').filter({ hasText: bob }).click();
    await a.getByRole('button', { name: 'Trade', exact: true }).click();
    const tradeA = a.getByTestId('trade');
    const tradeB = b.getByTestId('trade');
    await tradeB.getByRole('button', { name: 'Join trade', exact: true }).click();
    await expect(tradeA).toContainText('Preparing offers');
    await tradeA.getByLabel('Offer quantity: Bronze Blade', { exact: true }).fill('1');
    await tradeA.getByRole('button', { name: 'Send offer', exact: true }).click();
    await expect(tradeB).toContainText('Offer version: 1');
    await tradeB.getByLabel("Offer quantity: Recruit's Pauldron", { exact: true }).fill('1');
    await tradeB.getByRole('button', { name: 'Send offer', exact: true }).click();
    await expect(tradeA).toContainText('Offer version: 2');
    await tradeA.getByRole('button', { name: 'Accept current offers', exact: true }).click();
    await a.getByRole('dialog').getByRole('button', { name: 'Confirm trade', exact: true }).click();
    await expect(
      tradeA.getByRole('button', { name: 'Accept current offers', exact: true }),
    ).toBeDisabled();
    await tradeB.getByRole('button', { name: 'Accept current offers', exact: true }).click();
    await b.getByRole('dialog').getByRole('button', { name: 'Confirm trade', exact: true }).click();
    await expect(tradeA).toContainText('Trade completed');
    await expect(tradeB).toContainText('Trade completed');
    await a.screenshot({ path: `../target/trade-${info.project.name}.png`, fullPage: true });
    expect(
      await a.evaluate(
        () => document.documentElement.scrollWidth > document.documentElement.clientWidth,
      ),
    ).toBe(false);
    await progression(a, alice, info.project.name);
    await clans(a, b, alice, bob, info.project.name);
    await tournaments(a, b, alice, info.project.name);
    await expeditions(a, b, info.project.name);
    await administration(a, b, alice, bob, info.project.name);
    await privacy(b, bob, info.project.name);
  } finally {
    await first.close();
    await second.close();
  }
});
