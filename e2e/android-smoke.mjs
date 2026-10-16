// Run against the debug APK on a task-owned emulator. No production account or external API.
import { execFileSync } from 'node:child_process';
import { mkdir, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { _android, expect } from '@playwright/test';

const root = fileURLToPath(new URL('../', import.meta.url));
const adbPath = process.env.ADB ?? `${process.env.LOCALAPPDATA}/Android/Sdk/platform-tools/adb.exe`;
const serial = process.env.ANDROID_SERIAL ?? 'emulator-5554';
const pkg = process.env.ANDROID_PACKAGE ?? 'dev.game.training';
if (!/^dev\.game\.training(?:\.fixture)?$/.test(pkg))
  throw new Error('Only a disposable test package is allowed');
if (!serial.startsWith('emulator-'))
  throw new Error('This automated smoke test requires a task-owned emulator');
const adb = (...args) =>
  execFileSync(adbPath, ['-s', serial, ...args], { encoding: 'utf8', timeout: 20000 })
    .replaceAll('\r', '')
    .trim();
const pause = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
const wait = async (fn, description) => {
  const deadline = Date.now() + 10000;
  while (Date.now() < deadline) {
    if (await fn()) return;
    await pause(100);
  }
  throw new Error(description);
};
console.log('Starting the debug app on the emulator');
adb('shell', 'am', 'force-stop', pkg);
if (
  /Error:|Error type/.test(
    adb('shell', 'am', 'start', '-W', '-n', `${pkg}/dev.game.training.MainActivity`),
  )
)
  throw new Error(
    'Install the debug APK after instrumentation cleanup before running WebView tests',
  );
const device = (await _android.devices({ omitDriverInstall: true })).find(
  (value) => value.serial() === serial,
);
if (!device) throw new Error('Task emulator was not found');
device.setDefaultTimeout(10000);
let page;
try {
  page = await (await device.webView({ pkg })).page();
  page.setDefaultTimeout(10000);
  const errors = [],
    apiRequests = [];
  page.on('pageerror', (error) => errors.push(error.message));
  page.on('request', (request) => {
    if (new URL(request.url()).pathname.startsWith('/api/')) apiRequests.push(request.url());
  });
  await page.evaluate(() => {
    localStorage.setItem('game.language', 'en');
    localStorage.setItem('game.speed', 'instant');
  });
  await page.reload();
  await expect(page.getByRole('link', { name: 'Training', exact: true })).toBeVisible();
  const storage = await page.evaluate(() => JSON.stringify(Object.entries(localStorage).sort()));
  await page.getByRole('button', { name: 'Play as guest', exact: true }).click();
  await page.locator('header nav').getByRole('link', { name: 'Settings', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'App settings', exact: true })).toBeVisible();
  await expect(page.getByText('App version: 1.0.0', { exact: true })).toBeVisible();
  await expect(page.getByLabel('Vibrations during duels')).toBeChecked();
  await page.getByLabel('Vibrations during duels').uncheck();
  await expect(page.getByLabel('Reminder when leaving an ongoing duel')).toHaveCount(0);
  await expect(page.getByLabel('Keep the screen on during duels')).toBeChecked();
  adb('shell', 'svc', 'wifi', 'disable');
  adb('shell', 'svc', 'data', 'disable');
  await page.locator('header nav').getByRole('link', { name: 'Training', exact: true }).click();
  await page.getByRole('link', { name: 'Start training', exact: true }).click();
  await expect(page.getByRole('heading', { level: 1 })).toHaveText('Round 1 / 5');
  const round = await page.getByRole('heading', { level: 1 }).innerText();
  const awake = () =>
    page.evaluate(
      async () =>
        (await window.Capacitor.nativePromise('GamePlatform', 'metadata', {})).screenAwake,
    );
  await wait(awake, 'Duel did not enable the Android screen-on flag');
  await page.screenshot({ path: 'mobile/store/assets/offline-play.png', fullPage: false });
  console.log('Offline WASM training and native screen-on flag confirmed');
  adb('shell', 'input', 'keyevent', 'KEYCODE_HOME');
  await pause(1500);
  adb('shell', 'am', 'start', '-n', `${pkg}/dev.game.training.MainActivity`);
  await expect(page.getByRole('heading', { level: 1 })).toHaveText(round);
  const back = page.waitForEvent('dialog');
  adb('shell', 'input', 'keyevent', 'KEYCODE_BACK');
  const dialog = await back;
  expect(dialog.message()).toContain('local training');
  // CDP dismissal does not close Capacitor's native Android AlertDialog.
  adb('shell', 'uiautomator', 'dump', '/data/local/tmp/game-smoke.xml');
  const button = adb('shell', 'cat', '/data/local/tmp/game-smoke.xml').match(
    /resource-id="android:id\/button2"[^>]*bounds="\[(\d+),(\d+)\]\[(\d+),(\d+)\]"/,
  );
  if (!button) throw new Error('Android cancel button was not found');
  adb(
    'shell',
    'input',
    'tap',
    String((Number(button[1]) + Number(button[3])) >> 1),
    String((Number(button[2]) + Number(button[4])) >> 1),
  );
  await expect(page.getByRole('heading', { level: 1 })).toHaveText(round);
  await page.getByRole('button', { name: 'Forfeit duel', exact: true }).click();
  await page.getByRole('dialog').getByRole('button', { name: 'Forfeit duel', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Defeat', exact: true })).toBeVisible();
  await wait(async () => !(await awake()), 'Finished duel retained the Android screen-on flag');
  expect(await page.evaluate(() => JSON.stringify(Object.entries(localStorage).sort()))).toBe(
    storage,
  );
  expect(apiRequests).toEqual([]);
  expect(errors).toEqual([]);
  await mkdir(`${root}target`, { recursive: true });
  await page.screenshot({ path: `${root}target/f12a-android-offline.png`, fullPage: true });
  await writeFile(
    `${root}target/f12a-android-smoke.json`,
    `${JSON.stringify(
      {
        serial,
        offline_training: true,
        native_settings: true,
        guest_storage_unchanged: true,
        guest_api_requests: 0,
        background_resume: true,
        back_cancel: true,
        screen_on_only_during_duel: true,
        errors,
      },
      null,
      2,
    )}\n`,
  );
  console.log('Android offline, guest, settings, lifecycle and back-button checks passed');
} finally {
  adb('shell', 'svc', 'wifi', 'enable');
  adb('shell', 'svc', 'data', 'enable');
  await device.close();
}
