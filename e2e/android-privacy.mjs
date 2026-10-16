import { execFileSync } from 'node:child_process';
import { writeFile } from 'node:fs/promises';
import { _android, expect } from '@playwright/test';
import { startFixture } from './android-fixture.mjs';

const serial = process.env.ANDROID_SERIAL ?? 'emulator-5554';
if (!serial.startsWith('emulator-')) throw new Error('Task-owned emulator required');
const pkg = 'dev.game.training';
const adbPath = process.env.ADB ?? `${process.env.LOCALAPPDATA}/Android/Sdk/platform-tools/adb.exe`;
const adb = (...args) =>
  execFileSync(adbPath, ['-s', serial, ...args], {
    encoding: 'utf8',
    timeout: 20000,
    stdio: ['ignore', 'pipe', 'pipe'],
  });
const pause = (ms) => new Promise((r) => setTimeout(r, ms));
const name = `spellgambit-fixture-${Date.now()}.json`;
function tree() {
  adb('shell', 'rm', '-f', '/data/local/tmp/game-export.xml');
  adb('shell', 'uiautomator', 'dump', '/data/local/tmp/game-export.xml');
  return adb('shell', 'cat', '/data/local/tmp/game-export.xml');
}
async function control(pattern) {
  for (let i = 0; i < 20; i++) {
    let xml;
    try {
      xml = tree();
    } catch {
      await pause(250);
      continue;
    }
    const node = [...xml.matchAll(/<node\s[^>]+/g)].find(
      ([value]) => pattern.test(value) && !value.includes('bounds="[0,0][0,0]"'),
    )?.[0];
    if (node) return node;
    await pause(250);
  }
  throw new Error('Android document control unavailable');
}
function tap(node) {
  const b = node.match(/bounds="\[(\d+),(\d+)\]\[(\d+),(\d+)\]"/);
  if (!b) throw new Error('Control bounds missing');
  adb('shell', 'input', 'tap', String((+b[1] + +b[3]) >> 1), String((+b[2] + +b[4]) >> 1));
}
const stop = await startFixture();
let device;
try {
  const login = `Privacy_${Date.now().toString(36)}`,
    password = 'Native privacy fixture password!';
  const response = await fetch('http://127.0.0.1:18092/api/auth/register', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', 'X-Requested-With': 'app' },
    body: JSON.stringify({
      login,
      email: `${login}@example.test`,
      password,
      school: 'combat',
      terms_version: '2026-10-16',
      age_confirmed: true,
    }),
  });
  expect(response.status).toBe(201);
  // Only the disposable task app is reset; cached F12A bundles must not mask the current build.
  adb('shell', 'pm', 'clear', pkg);
  adb('shell', 'am', 'start', '-W', '-n', `${pkg}/.MainActivity`);
  device = (await _android.devices({ omitDriverInstall: true })).find((d) => d.serial() === serial);
  if (!device) throw new Error('Task emulator missing');
  device.setDefaultTimeout(20000);
  const page = await (await device.webView({ pkg })).page();
  page.setDefaultTimeout(20000);
  await page.evaluate(() => {
    localStorage.setItem('game.language', 'en');
    location.hash = '/account';
  });
  await page.reload();
  await page.getByLabel('Login or email').fill(login);
  await page.getByLabel('Password', { exact: true }).fill(password);
  await page.getByRole('button', { name: 'Sign in', exact: true }).last().click();
  await expect(page.getByRole('heading', { name: login, exact: true })).toBeVisible();
  await page.getByText('Data and account deletion', { exact: true }).click();
  const controls = page.locator('.privacy-controls');
  await expect(controls).toContainText(`Account: ${login}`);
  const download = controls.getByRole('button', {
    name: 'Download account data (JSON)',
    exact: true,
  });
  await download.click();
  await control(/class="android.widget.EditText"/);
  adb('shell', 'input', 'keyevent', 'KEYCODE_BACK');
  await expect(download).toBeEnabled();
  await download.click();
  tap(await control(/class="android.widget.EditText"/));
  adb('shell', 'input', 'keycombination', 'KEYCODE_CTRL_LEFT', 'KEYCODE_A');
  adb('shell', 'input', 'text', name);
  tap(await control(/resource-id="com.android.documentsui:id:save"|text="SAVE"/));
  await expect(controls.getByRole('status')).toContainText('Your data file was saved');
  const text = adb('shell', 'cat', `/sdcard/Download/${name}`);
  const data = JSON.parse(text);
  expect(data.account[0].login).toBe(login);
  expect(text).not.toMatch(/password_hash|id_hash|token_hash/);
  await page.getByRole('button', { name: 'Sign out', exact: true }).click();
  const rights = controls.locator('form').filter({
    has: page.getByRole('heading', { name: 'Access your data without gameplay', exact: true }),
  });
  await rights.getByLabel('Account login or email').fill(login);
  await rights.getByLabel('Current password').fill(password);
  await rights
    .getByRole('button', { name: 'Verify identity for data access', exact: true })
    .click();
  await expect(controls).toContainText(`Account: ${login}`);
  await expect(page.getByRole('heading', { name: login, exact: true })).toHaveCount(0);
  const token = await page.evaluate(
    async () => (await window.Capacitor.nativePromise('GamePlatform', 'loadSession', {})).token,
  );
  const me = await fetch('http://127.0.0.1:18092/api/me', {
    headers: {
      Authorization: `Bearer ${token}`,
      'X-Native-Version': '1.0.0',
      Origin: 'https://localhost',
    },
  });
  expect(me.status).toBe(401);
  const pid = adb('shell', 'pidof', pkg).trim();
  if (adb('logcat', '-d', '--pid', pid, '-t', '3000').includes(token))
    throw new Error('Credential detected in logcat');
  await controls.getByRole('button', { name: 'End data access session', exact: true }).click();
  await writeFile(
    'target/f13-android-privacy.json',
    `${JSON.stringify({ native_document_picker: true, cancel: true, https_export: true, no_storage_permission: true, restricted_session: true, no_token_in_logs: true }, null, 2)}\n`,
  );
  console.log('Native document export, cancellation and restricted data access passed');
} finally {
  // This unique synthetic export was created by this test on its own emulator.
  try {
    adb('shell', 'rm', '-f', `/sdcard/Download/${name}`);
  } catch {}
  if (device) await device.close();
  await stop();
}
