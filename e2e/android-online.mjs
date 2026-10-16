import { execFileSync } from 'node:child_process';
import { writeFile } from 'node:fs/promises';
import { _android, expect } from '@playwright/test';
import { startFixture } from './android-fixture.mjs';

const serial = process.env.ANDROID_SERIAL ?? 'emulator-5554',
  pkg = 'dev.game.training';
if (!serial.startsWith('emulator-'))
  throw new Error('Only a task-owned emulator may run this fixture');
const adbPath = process.env.ADB ?? `${process.env.LOCALAPPDATA}/Android/Sdk/platform-tools/adb.exe`;
const adb = (...args) =>
  execFileSync(adbPath, ['-s', serial, ...args], { encoding: 'utf8', timeout: 20000 });
const pause = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
const stop = await startFixture();
let device;
try {
  const login = `Android_${Date.now().toString(36)}`,
    password = 'Native disposable test password!';
  const headers = {
    'Content-Type': 'application/json',
    'X-Requested-With': 'app',
    'X-Native-Version': '1.0.0',
    Origin: 'https://localhost',
  };
  const post = async (path, body) => {
    const response = await fetch(`http://127.0.0.1:18092${path}`, {
      method: 'POST',
      headers,
      body: JSON.stringify(body),
    });
    if (!response.ok) throw new Error(`Fixture ${path}: HTTP ${response.status}`);
    return response;
  };
  await post('/api/auth/register', {
    login,
    email: `${login}@example.test`,
    password,
    school: 'combat',
    terms_version: '2026-10-16',
    age_confirmed: true,
    client: 'android',
  });
  const mail = await (await fetch(`http://127.0.0.1:18092/__mail?to=${login}@example.test`)).json();
  const link = mail[0].body.match(/https?:\/\/\S+/)[0];
  const token = new URLSearchParams(link.split('?')[1]).get('token');
  await post('/api/auth/verify-email', { token });
  adb('shell', 'am', 'force-stop', pkg);
  adb('shell', 'am', 'start', '-W', '-n', `${pkg}/.MainActivity`);
  device = (await _android.devices({ omitDriverInstall: true })).find((d) => d.serial() === serial);
  if (!device) throw new Error('Task emulator missing');
  device.setDefaultTimeout(15000);
  const page = await (await device.webView({ pkg })).page();
  page.setDefaultTimeout(15000);
  await page.evaluate(() => {
    localStorage.setItem('game.language', 'en');
    location.hash = '/account';
  });
  await page.reload();
  // Clear any credential belonging to a previous disposable fixture, then use real UI login.
  await page.evaluate(async () =>
    window.Capacitor.nativePromise('GamePlatform', 'clearSession', {}),
  );
  await page.reload();
  await page.getByLabel('Login or email').fill(login);
  await page.getByLabel('Password', { exact: true }).fill(password);
  await page.getByRole('button', { name: 'Sign in', exact: true }).last().click();
  await expect(page.getByRole('heading', { name: login, exact: true })).toBeVisible();
  const stored = await page.evaluate(async () => {
    const v = await window.Capacitor.nativePromise('GamePlatform', 'loadSession', {});
    return !!v.token && v.token.length === 64;
  });
  expect(stored).toBe(true);
  const sessionToken = await page.evaluate(
    async () => (await window.Capacitor.nativePromise('GamePlatform', 'loadSession', {})).token,
  );
  const pid = adb('shell', 'pidof', pkg).trim();
  if (adb('logcat', '-d', '--pid', pid, '-t', '3000').includes(sessionToken))
    throw new Error('Credential detected in Android logs');
  let rawTokenInURL = false;
  page.on('websocket', (socket) => {
    if (socket.url().includes(sessionToken)) rawTokenInURL = true;
  });
  const enter = async () => {
    await page.locator('header nav').getByRole('link', { name: 'Lobby', exact: true }).click();
    await expect(page.getByTestId('connection-status')).toHaveText('Connected');
    await page.getByRole('button', { name: 'Training', exact: true }).click();
    await expect(page.locator('.online-bots button')).toHaveCount(8);
    await page.locator('.online-bots button').first().click();
    await expect(page.getByTestId('online-duel')).toContainText('Opponent ready');
  };
  await enter();
  const duel = page.getByTestId('online-duel');
  await page.screenshot({ path: 'mobile/store/assets/online-play.png', fullPage: false });
  const round = await duel.getByRole('heading').first().innerText();
  console.log('Native TLS login, one-time WS ticket and online bot duel confirmed');
  adb('shell', 'input', 'keyevent', 'KEYCODE_HOME');
  await pause(3000);
  adb('shell', 'am', 'start', '-W', '-n', `${pkg}/.MainActivity`);
  await expect(page.getByTestId('connection-status')).toHaveText('Connected');
  await expect(duel.getByRole('heading').first()).toHaveText(round);
  await duel.getByRole('button', { name: 'Surrender', exact: true }).click();
  await page.getByRole('dialog').getByRole('button', { name: 'Surrender', exact: true }).click();
  await expect(duel.locator('.online-result')).toBeVisible();
  await duel.getByRole('button', { name: 'Close', exact: true }).click();
  await page.locator('.online-bots button').first().click();
  await expect(duel).toContainText('Opponent ready');
  console.log('Short background/resume passed; checking the 60-second disconnect rule');
  adb('shell', 'input', 'keyevent', 'KEYCODE_HOME');
  await pause(65000);
  adb('shell', 'am', 'start', '-W', '-n', `${pkg}/.MainActivity`);
  await expect(page.getByTestId('connection-status')).toHaveText('Connected');
  await expect(duel.locator('.online-result')).toContainText('Defeat');
  await expect(duel.locator('.online-result')).toContainText('Disconnection');
  await page.screenshot({ path: 'target/f12a-android-online.png', fullPage: true });
  const raw = await page.evaluate(() => JSON.stringify(Object.entries(localStorage)));
  expect(raw).not.toContain(password);
  expect(raw).not.toMatch(/[a-f0-9]{64}/);
  await page.locator('header nav').getByRole('link', { name: 'Account', exact: true }).click();
  await page.getByRole('button', { name: 'Sign out', exact: true }).click();
  if (rawTokenInURL) throw new Error('Credential detected in a WebSocket URL');
  await writeFile(
    'target/f12a-android-online.json',
    `${JSON.stringify(
      {
        tls_verified: true,
        native_login: true,
        keystore: true,
        ws_ticket: true,
        resume_under_60s: true,
        defeat_after_60s: true,
        logout: true,
      },
      null,
      2,
    )}\n`,
  );
  console.log('Android online lifecycle checks passed');
} finally {
  if (device) await device.close();
  await stop();
}
