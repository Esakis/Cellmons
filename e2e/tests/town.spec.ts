import { expect, type Page, test } from '@playwright/test';
import { chooseLanguage } from '../language';

async function checkMap(page: Page, name: string) {
  await expect(page.locator('.town-place')).toHaveCount(10);
  // After a visit to Settings the map is mounted again; wait for its painting.
  await expect
    .poll(() =>
      page
        .locator('.town-canvas > img')
        .evaluate((image: HTMLImageElement) => image.complete && image.naturalWidth > 0),
    )
    .toBe(true);
  const geometry = await page.locator('.town').evaluate((town) => {
    const canvas = town.querySelector('.town-canvas') as HTMLElement;
    const image = canvas.querySelector('img') as HTMLImageElement;
    const bounds = canvas.getBoundingClientRect();
    const navigation = document.querySelector('header nav') as HTMLElement;
    const markers = [...town.querySelectorAll('.town-place')].map((node) => ({
      id: node.getAttribute('data-place'),
      position: getComputedStyle(node).position,
      ...node.getBoundingClientRect().toJSON(),
    }));
    return {
      bounds: bounds.toJSON(),
      markers,
      loaded: image.complete && image.naturalWidth > 0,
      width: innerWidth,
      height: innerHeight,
      bottomLimit:
        getComputedStyle(navigation).position === 'fixed'
          ? navigation.getBoundingClientRect().top
          : innerHeight,
      scrollWidth: document.documentElement.scrollWidth,
      scrollHeight: document.documentElement.scrollHeight,
    };
  });
  await page.screenshot({ path: `../target/map-redesign/${name}.png`, fullPage: true });
  expect(geometry.loaded).toBe(true);
  expect(geometry.scrollWidth).toBeLessThanOrEqual(geometry.width);
  expect(geometry.scrollHeight).toBeLessThanOrEqual(geometry.height + 1);
  for (const marker of geometry.markers) {
    expect(marker.position, marker.id).toBe('absolute');
    expect(marker.height, marker.id).toBeGreaterThanOrEqual(44);
    expect(marker.left, marker.id).toBeGreaterThanOrEqual(geometry.bounds.left);
    expect(marker.right, marker.id).toBeLessThanOrEqual(geometry.bounds.right);
    expect(marker.top, marker.id).toBeGreaterThanOrEqual(geometry.bounds.top);
    expect(marker.bottom, marker.id).toBeLessThanOrEqual(geometry.bounds.bottom);
    expect(marker.bottom, marker.id).toBeLessThanOrEqual(geometry.bottomLimit);
    for (const other of geometry.markers) {
      if (other.id === marker.id) continue;
      const overlap =
        marker.left < other.right &&
        marker.right > other.left &&
        marker.top < other.bottom &&
        marker.bottom > other.top;
      expect(overlap, `${marker.id} overlaps ${other.id}`).toBe(false);
    }
  }
}

test('all town destinations stay on the painting, fit the screen and open the right places', async ({
  page,
}, info) => {
  test.setTimeout(120000);
  const login = `Map_${info.project.name[0]}${Date.now().toString(36)}`;
  await page.addInitScript(() => localStorage.setItem('game.language', 'en'));
  const created = await page.request.post('http://127.0.0.1:18092/__account', {
    data: {
      login,
      email: `${login}@example.test`,
      password: 'Map integration password!',
      school: 'combat',
      template_id: 1,
      terms_version: '2026-10-16',
      age_confirmed: true,
    },
  });
  expect(created.status()).toBe(201);
  await page.goto('/#/account');
  await page.getByLabel('Login or email').fill(login);
  await page.getByLabel('Password', { exact: true }).fill('Map integration password!');
  await page.getByRole('button', { name: 'Sign in', exact: true }).last().click();
  await expect(page.getByRole('heading', { name: login })).toBeVisible();
  await page.locator('header nav').getByRole('link', { name: 'Lobby', exact: true }).click();
  await expect(page.getByTestId('connection-status')).toHaveText('Connected');

  const sizes = info.project.name.startsWith('mobile')
    ? [
        { width: 320, height: 568 },
        { width: 390, height: 844 },
        { width: 412, height: 915 },
        { width: 844, height: 390 },
        { width: 640, height: 360 },
      ]
    : [
        { width: 1280, height: 720 },
        { width: 768, height: 1024 },
      ];
  for (const size of sizes) {
    await page.setViewportSize(size);
    await checkMap(page, `new-${size.width}`);
  }
  await expect(page.locator('.town-place[data-place=ranked]')).toBeDisabled();
  await expect(page.locator('.town-place[data-place=training]')).toBeEnabled();
  await page.request.post('http://127.0.0.1:18092/__journey', { data: { login, xp: 42000 } });
  await page.reload();
  await expect(page.getByTestId('connection-status')).toHaveText('Connected');
  for (const size of sizes) {
    await page.setViewportSize(size);
    for (const language of ['en', 'pl'] as const) {
      await chooseLanguage(page, language);
      await checkMap(page, `${language}-${size.width}`);
    }
  }
  await chooseLanguage(page, 'en');
  // The painted training courtyard itself is also a hit target, not just its label.
  await page.locator('.town-shape').nth(8).click();
  await expect(page.getByRole('heading', { name: 'Sparring', exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Map', exact: true }).click();
  for (const [place, title] of [
    ['training', 'Sparring'],
    ['arena', 'Arena'],
    ['ranked', 'Ranked Arena'],
    ['castle', 'Castle'],
    ['portal', 'Portal'],
    ['bazaar', 'Bazaar'],
  ]) {
    await page.locator(`.town-place[data-place=${place}]`).click();
    await expect(page.getByRole('heading', { name: title, exact: true })).toBeVisible();
    await page.getByRole('button', { name: 'Map', exact: true }).click();
  }
  // The library, the shop and the tower (and the clan hall without a clan) are pages.
  for (const [place, route] of [
    ['library', '/collection'],
    ['shop', '/store'],
    ['tower', '/clans'],
    ['clan', '/clans'],
  ]) {
    await page.locator(`.town-place[data-place=${place}]`).click();
    await expect(page).toHaveURL(new RegExp(`${route}$`));
    await page.locator('header nav').getByRole('link', { name: 'Lobby', exact: true }).click();
    await expect(page.getByTestId('connection-status')).toHaveText('Connected');
  }
  await page.locator('header nav').getByRole('link', { name: 'Training', exact: true }).click();
  const vampire = page.getByRole('button', { name: /Vampire/ }).locator('img');
  await vampire.scrollIntoViewIfNeeded();
  await expect(vampire).toHaveAttribute('src', /vampire-[\w-]+\.avif$/);
  await expect
    .poll(() =>
      vampire.evaluate((img: HTMLImageElement) => img.complete && img.naturalWidth === 576),
    )
    .toBe(true);
});
