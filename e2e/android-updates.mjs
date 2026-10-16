import { execFileSync } from 'node:child_process';
import { cp, mkdir, readFile, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { _android, expect } from '@playwright/test';
import { fixture, startFixture } from './android-fixture.mjs';

const serial = process.env.ANDROID_SERIAL ?? 'emulator-5554',
  pkg = 'dev.game.training';
if (!serial.startsWith('emulator-'))
  throw new Error('Only a task-owned emulator may run this fixture');
const adbPath = process.env.ADB ?? `${process.env.LOCALAPPDATA}/Android/Sdk/platform-tools/adb.exe`;
const adb = (...args) =>
  execFileSync(adbPath, ['-s', serial, ...args], { encoding: 'utf8', timeout: 20000 });
const pause = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
const output = `${fixture}/cdn`;
await mkdir(output, { recursive: true });
function pack(version, min = 10000, input = 'web/dist') {
  execFileSync(process.execPath, ['scripts/pack-web-bundle.mjs'], {
    env: {
      ...process.env,
      WEB_BUNDLE_VERSION: String(version),
      ANDROID_VERSION_CODE: String(min),
      WEB_BUNDLE_INPUT: resolve(input),
      WEB_BUNDLE_OUTPUT: output,
      WEB_BUNDLE_SIGNING_KEY: `${fixture}/bundle-private.pem`,
    },
    stdio: 'pipe',
  });
}
pack(1);
const stop = await startFixture({ bundles: true });
let device, page;
try {
  adb('shell', 'pm', 'clear', pkg);
  const login = `Updater_${Date.now().toString(36)}`,
    password = 'Update disposable test password!';
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
    if (!response.ok) throw new Error(`Fixture HTTP ${response.status}`);
    return response.json();
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
  await post('/api/auth/verify-email', {
    token: new URLSearchParams(mail[0].body.match(/https?:\/\/\S+/)[0].split('?')[1]).get('token'),
  });
  const credentials = await post('/api/auth/login', {
    identity: login,
    password,
    client: 'android',
  });
  adb('shell', 'am', 'force-stop', pkg);
  adb('shell', 'am', 'start', '-W', '-n', `${pkg}/.MainActivity`);
  device = (await _android.devices({ omitDriverInstall: true })).find((d) => d.serial() === serial);
  if (!device) throw new Error('Task emulator missing');
  device.setDefaultTimeout(10000);
  page = await (await device.webView({ pkg })).page();
  await page.evaluate(async (token) => {
    localStorage.setItem('game.language', 'en');
    await window.Capacitor.nativePromise('GamePlatform', 'saveSession', {
      token,
      rotatedAt: Date.now(),
    });
  }, credentials.token);
  const restart = async () => {
    const closed = page.waitForEvent('close', { timeout: 5000 }).catch(() => {});
    adb('shell', 'am', 'force-stop', pkg);
    await closed;
    await pause(1100);
    adb('shell', 'am', 'start', '-W', '-n', `${pkg}/.MainActivity`);
    page = await (await device.webView({ pkg })).page();
    page.setDefaultTimeout(10000);
  };
  const metadata = async () => {
    try {
      return await page.evaluate(async () =>
        window.Capacitor.nativePromise('GamePlatform', 'metadata', {}),
      );
    } catch (error) {
      if (error.message.includes('Execution context was destroyed'))
        return { bundleVersion: -1, bundleTrial: true };
      throw error;
    }
  };
  const version = async () => (await metadata()).bundleVersion;
  const settle = () =>
    page.evaluate(async () => {
      try {
        await window.Capacitor.nativePromise('GamePlatform', 'checkBundle', {});
      } catch {}
    });
  await restart();
  await expect.poll(version, { timeout: 20000 }).toBe(1);
  await expect.poll(async () => (await metadata()).bundleTrial, { timeout: 20000 }).toBe(false);
  await expect(page.getByRole('link', { name: 'Training', exact: true })).toBeVisible();
  console.log('Authentic bundle downloaded, signature verified and healthy WASM boot confirmed');
  pack(2);
  const damaged = await readFile(`${output}/2.zip`);
  damaged[damaged.length - 30] ^= 1;
  await writeFile(`${output}/2.zip`, damaged);
  await restart();
  await settle();
  expect(await version()).toBe(1);
  pack(3);
  const badSignature = JSON.parse(await readFile(`${output}/manifest.json`, 'utf8'));
  const bytes = Buffer.from(badSignature.signature, 'base64');
  bytes[0] ^= 1;
  badSignature.signature = bytes.toString('base64');
  await writeFile(`${output}/manifest.json`, JSON.stringify(badSignature));
  await restart();
  await settle();
  expect(await version()).toBe(1);
  pack(4, 2000000);
  await restart();
  await settle();
  expect(await version()).toBe(1);
  console.log('Corrupt, incorrectly signed and incompatible bundles rejected');
  await cp('web/dist', `${fixture}/broken-web`, { recursive: true });
  await writeFile(
    `${fixture}/broken-web/index.html`,
    '<!doctype html><html><head><title>Failed boot fixture</title></head><body><main>Intentionally missing WASM health acknowledgement</main></body></html>',
  );
  pack(5, 10000, `${fixture}/broken-web`);
  await restart();
  await expect.poll(version, { timeout: 20000 }).toBe(5);
  await expect.poll(version, { timeout: 20000 }).toBe(1);
  await expect(page.getByRole('link', { name: 'Training', exact: true })).toBeVisible();
  await settle();
  expect(await version()).toBe(1);
  await page.evaluate(async () =>
    window.Capacitor.nativePromise('GamePlatform', 'clearSession', {}),
  );
  await writeFile(
    'target/f12a-android-updates.json',
    `${JSON.stringify(
      {
        authentic: true,
        sha256: true,
        wrong_signature_rejected: true,
        incompatible_skipped: true,
        failed_boot_rolled_back: true,
        failed_version_not_retried: true,
      },
      null,
      2,
    )}\n`,
  );
  console.log('Native signed update and watchdog rollback checks passed');
} finally {
  if (device) await device.close();
  await stop();
}
