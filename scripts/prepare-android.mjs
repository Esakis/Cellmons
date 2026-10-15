import { spawnSync } from 'node:child_process';
import { readFile, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('../', import.meta.url));
// Use the pnpm CLI through Node so paths with spaces work on Windows too.
const pnpm = process.env.npm_execpath;
if (!pnpm) throw new Error('Run with pnpm --filter mobile sync');
for (const [cwd, args] of [
  [root, ['--filter', 'web', 'build']],
  [fileURLToPath(new URL('../mobile', import.meta.url)), ['exec', 'cap', 'sync', 'android']],
]) {
  const native = pnpm.endsWith('.exe');
  const result = spawnSync(native ? pnpm : process.execPath, native ? args : [pnpm, ...args], {
    cwd,
    env: { ...process.env, VITE_PLATFORM: 'android' },
    stdio: 'inherit',
  });
  if (result.status !== 0) process.exit(result.status ?? 1);
}
// cap sync resolves the pnpm store on Windows; retain a portable project path.
const settings = new URL('../mobile/android/capacitor.settings.gradle', import.meta.url);
await writeFile(
  settings,
  (await readFile(settings, 'utf8')).replace(
    /new File\('[^']+@capacitor\/android\/capacitor'\)/,
    "new File('../node_modules/@capacitor/android/capacitor')",
  ),
);
