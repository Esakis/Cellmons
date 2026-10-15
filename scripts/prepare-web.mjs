// Reproducible browser assets from the same content and Rust engine as the server.
import { spawnSync } from 'node:child_process';
import { cp, mkdir, readdir, readFile, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { optimize } from 'svgo';

const root = fileURLToPath(new URL('../', import.meta.url));
process.chdir(root);
function run(command, args) {
  const result = spawnSync(command, args, { stdio: 'inherit' });
  if (result.status !== 0) process.exit(result.status ?? 1);
}
run('cargo', ['run', '-q', '-p', 'content-compiler', '--locked', '--', 'build']);
await mkdir('web/src/generated', { recursive: true });
await cp('target/content/content.json', 'web/src/generated/content.json');
for (const locale of ['pl', 'en']) {
  await cp(`target/content/i18n.${locale}.ftl`, `web/src/generated/${locale}.ftl`);
}
for (const group of ['avatars', 'icons', 'frames', 'cards']) {
  await mkdir(`web/src/assets/${group}`, { recursive: true });
  for (const name of await readdir(`art/${group}`)) {
    if (!name.endsWith('.svg')) continue;
    const svg = await readFile(`art/${group}/${name}`, 'utf8');
    const result = optimize(svg, { multipass: true });
    await writeFile(`web/src/assets/${group}/${name}`, result.data);
  }
}
run('cargo', [
  'build',
  '-p',
  'engine-wasm',
  '--target',
  'wasm32-unknown-unknown',
  '--profile',
  'wasm-release',
  '--locked',
]);
run('wasm-bindgen', [
  'target/wasm32-unknown-unknown/wasm-release/engine_wasm.wasm',
  '--target',
  'web',
  // Ship executable code; Rust symbol names remain in the local Cargo artifact for debugging.
  '--remove-name-section',
  '--remove-producers-section',
  '--out-dir',
  'web/src/generated/engine',
]);
