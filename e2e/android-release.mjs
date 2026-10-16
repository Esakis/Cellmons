// Release WebView debugging stays disabled. Exercise the signed APK via Android accessibility.
import { execFileSync } from 'node:child_process';
import { writeFile } from 'node:fs/promises';

const adbPath = process.env.ADB ?? `${process.env.LOCALAPPDATA}/Android/Sdk/platform-tools/adb.exe`;
const serial = process.env.ANDROID_SERIAL ?? 'emulator-5554',
  pkg = 'dev.game.training.fixture';
if (!serial.startsWith('emulator-')) throw new Error('Disposable emulator required');
const adb = (...args) =>
  execFileSync(adbPath, ['-s', serial, ...args], {
    encoding: 'utf8',
    timeout: 20000,
    stdio: ['ignore', 'pipe', 'pipe'],
  });
const pause = (ms) => new Promise((r) => setTimeout(r, ms));
function tree() {
  adb('shell', 'rm', '-f', '/data/local/tmp/game-release.xml');
  adb('shell', 'uiautomator', 'dump', '/data/local/tmp/game-release.xml');
  return adb('shell', 'cat', '/data/local/tmp/game-release.xml');
}
async function find(pattern) {
  for (let i = 0; i < 18; i++) {
    let xml;
    try {
      xml = tree();
    } catch {
      await pause(250);
      continue;
    }
    const node = [...xml.matchAll(/<node\s[^>]+/g)].find((value) => {
      const b = value[0].match(/bounds="\[(\d+),(\d+)\]\[(\d+),(\d+)\]"/);
      return (
        pattern.test(value[0]) &&
        b &&
        +b[3] - +b[1] > 10 &&
        +b[4] - +b[2] > 10 &&
        (+b[2] + +b[4]) / 2 > 75 &&
        (+b[2] + +b[4]) / 2 < 2100
      );
    });
    if (node) return node[0];
    adb('shell', 'input', 'swipe', '540', '1700', '540', '700', '400');
    await pause(200);
  }
  throw new Error(`Release control missing: ${pattern}`);
}
function tap(node) {
  const b = node.match(/bounds="\[(\d+),(\d+)\]\[(\d+),(\d+)\]"/);
  if (!b) throw new Error('Missing bounds');
  adb('shell', 'input', 'tap', String((+b[1] + +b[3]) >> 1), String((+b[2] + +b[4]) >> 1));
}
try {
  adb('shell', 'am', 'force-stop', pkg);
  adb('shell', 'am', 'start', '-W', '-n', `${pkg}/dev.game.training.MainActivity`);
  if (!adb('shell', 'pidof', pkg).trim()) throw new Error('Signed release failed to launch');
  adb('shell', 'svc', 'wifi', 'disable');
  adb('shell', 'svc', 'data', 'disable');
  tap(await find(/(?:text|content-desc)="(?:Play as guest|Graj jako gość)"/));
  tap(await find(/(?:text|content-desc)="(?:Start training|Rozpocznij trening)"/));
  await find(/(?:text|content-desc)="(?:Round 1 \/ 5|Runda 1 \/ 5)"/);
  const png = execFileSync(adbPath, ['-s', serial, 'exec-out', 'screencap', '-p']);
  await writeFile('target/f12a-release-offline.png', png);
  await writeFile(
    'target/f12a-release-ui.json',
    `${JSON.stringify(
      {
        fixture_package: pkg,
        signed_release: true,
        r8: true,
        offline_wasm: true,
        webview_debugging: false,
      },
      null,
      2,
    )}\n`,
  );
  console.log('Signed release APK started offline WASM training with WebView debugging disabled');
} finally {
  adb('shell', 'svc', 'wifi', 'enable');
  adb('shell', 'svc', 'data', 'enable');
}
