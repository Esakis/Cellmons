import { expect, test } from '@playwright/test';

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    localStorage.setItem('game.language', 'en');
    localStorage.setItem('game.speed', 'instant');
  });
});

test('R-KNT-04: guest completes offline training without API, cookies or persistent player data', async ({
  page,
  context,
}) => {
  let requests = 0;
  await page.route('**/api/**', (route) => {
    requests++;
    return route.abort();
  });
  await page.goto('/');
  await expect(page.getByTestId('offline-status')).toHaveText('Ready to play offline');
  await page.evaluate(async () => {
    await navigator.serviceWorker.ready;
    if (!navigator.serviceWorker.controller)
      await new Promise<void>((resolve) =>
        navigator.serviceWorker.addEventListener('controllerchange', () => resolve(), {
          once: true,
        }),
      );
  });
  const storage = await page.evaluate(async () => ({
    local: { ...localStorage },
    session: { ...sessionStorage },
    databases: await indexedDB.databases(),
  }));
  const cookies = await context.cookies();
  await context.setOffline(true);
  await page.getByRole('button', { name: 'Play as guest' }).click();
  await expect(page.getByRole('status').filter({ hasText: 'Playing as guest.' })).toBeVisible();
  await page.getByRole('button', { name: /Vampire/ }).click();
  await page.getByRole('button', { name: /Archmage/ }).click();
  await page.getByRole('link', { name: 'Settings', exact: true }).click();
  await page.getByRole('combobox', { name: 'Theme' }).selectOption('light');
  await page.locator('header nav').getByRole('link', { name: 'Training', exact: true }).click();
  await page.getByRole('link', { name: 'Watch bots play' }).click();
  const next = page.getByRole('button', { name: 'Next bot round' });
  for (let round = 0; round < 5; round++) {
    if (await page.locator('#duel-result').count()) break;
    await expect(next).toBeEnabled();
    await next.click();
    await expect(next.or(page.getByRole('button', { name: 'Play again' }))).toBeEnabled();
  }
  await expect(page.locator('#duel-result')).toBeVisible();
  expect(
    await page.evaluate(async () => ({
      local: { ...localStorage },
      session: { ...sessionStorage },
      databases: await indexedDB.databases(),
    })),
  ).toEqual(storage);
  expect(await context.cookies()).toEqual(cookies);
  expect(requests).toBe(0);
  await page.goto('/');
  await page.reload();
  await expect(page.getByRole('button', { name: 'Play as guest' })).toBeVisible();
  await expect(page.getByRole('status').filter({ hasText: 'Playing as guest.' })).toHaveCount(0);
});

test('account forms register, sign in and sign out using the API contract', async ({
  page,
}, info) => {
  const profile = {
    id: 'test-user',
    login: 'PlayerOne',
    email: 'one@example.test',
    email_verified: false,
    start_school: 'combat',
    role: 'player',
    avatar: { id: 'avatar', template_id: 1, level: 1, xp: 0 },
  };
  let loggedIn = false;
  let registered = false;
  await page.route('**/api/**', async (route) => {
    const path = new URL(route.request().url()).pathname;
    if (path === '/api/me')
      return route.fulfill({
        status: loggedIn ? 200 : 401,
        json: loggedIn ? profile : { error: 'UNAUTHENTICATED' },
      });
    if (path === '/api/auth/options') return route.fulfill({ json: { captcha_site_key: null } });
    if (path === '/api/auth/register') {
      expect(route.request().postDataJSON()).toMatchObject({
        login: 'PlayerOne',
        email: 'one@example.test',
        school: 'combat',
        client: 'web',
      });
      registered = true;
      return route.fulfill({ status: 201, json: { registered: true } });
    }
    if (path === '/api/auth/login') {
      expect(route.request().headers()['x-requested-with']).toBe('app');
      loggedIn = true;
      return route.fulfill({ json: { profile } });
    }
    if (path === '/api/auth/sessions') return route.fulfill({ json: { sessions: [] } });
    if (path === '/api/auth/logout') {
      loggedIn = false;
      return route.fulfill({ status: 204 });
    }
    return route.fulfill({ status: 404, json: { error: 'NOT_FOUND' } });
  });
  await page.goto('/#/account');
  await page.getByRole('button', { name: 'Register', exact: true }).click();
  await page.getByLabel('Login (3–20 letters, digits, _ or -)').fill('PlayerOne');
  await page.getByLabel('Email', { exact: true }).fill('one@example.test');
  await page.getByLabel('Password', { exact: true }).fill('A long test password!');
  await expect(page.locator('select[name=template] option')).toHaveCount(3);
  await page.locator('select[name=template]').selectOption('1');
  await page.screenshot({ path: `../target/f62-${info.project.name}-account.png`, fullPage: true });
  await page.locator('input[name=agreement]').check();
  await page.getByRole('button', { name: 'Create account', exact: true }).click();
  await expect(page.getByRole('status').filter({ hasText: 'Account created.' })).toBeVisible();
  expect(registered).toBe(true);
  await page.getByLabel('Login or email').fill('PlayerOne');
  await page.getByLabel('Password', { exact: true }).fill('A long test password!');
  await page.getByRole('button', { name: 'Sign in', exact: true }).last().click();
  await expect(page.getByRole('heading', { name: 'PlayerOne' })).toBeVisible();
  await page.getByRole('button', { name: 'Sign out', exact: true }).click();
  await expect(page.getByLabel('Login or email')).toBeVisible();
  expect(
    await page.evaluate(() =>
      Object.values(localStorage).some((value) => /test-user|password|one@example/.test(value)),
    ),
  ).toBe(false);
});

test('email link token is removed from the address and consumed only on confirmation', async ({
  page,
}) => {
  const token = 'a'.repeat(64);
  let consumed = false;
  await page.route('**/api/**', (route) => {
    if (new URL(route.request().url()).pathname === '/api/auth/verify-email') {
      consumed = true;
      expect(route.request().postDataJSON()).toEqual({ token });
      return route.fulfill({ json: { verified: true } });
    }
    return route.fulfill({ status: 401, json: { error: 'UNAUTHENTICATED' } });
  });
  // The email may be opened while the account route is already mounted.
  await page.goto('/#/account');
  await expect(page.getByRole('button', { name: 'Sign in', exact: true }).last()).toBeVisible();
  await page.goto(`/#/account?action=verify&token=${token}`);
  await expect(page).not.toHaveURL(/token=/);
  expect(consumed).toBe(false);
  await page.getByRole('button', { name: 'Verify email', exact: true }).click();
  await expect(
    page.getByRole('status').filter({ hasText: 'Email address verified.' }),
  ).toBeVisible();
  expect(consumed).toBe(true);
});
