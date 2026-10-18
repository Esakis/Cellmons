// Reproducible browser assets from the same content and Rust engine as the server.
import { spawnSync } from 'node:child_process';
import { cp, mkdir, readdir, readFile, rm, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { optimize } from 'svgo';

const root = fileURLToPath(new URL('../', import.meta.url));
process.chdir(root);
function run(command, args) {
  const result = spawnSync(command, args, { stdio: 'inherit' });
  if (result.status !== 0) process.exit(result.status ?? 1);
}
run('cargo', ['run', '-q', '-p', 'content-compiler', '--locked', '--', 'build']);
run('node', ['scripts/generate-training-wire.mjs']);
run('node', ['scripts/prepare-legal.mjs']);
run('node', ['scripts/collect-notices.mjs']);
await mkdir('web/src/generated', { recursive: true });
const { paths, cards, ...core } = JSON.parse(await readFile('target/content/content.json', 'utf8'));
await writeFile('web/src/generated/content.json', JSON.stringify(core));
await writeFile('web/src/generated/cards.json', JSON.stringify({ version: core.version, cards }));
await writeFile('web/src/generated/paths.json', JSON.stringify({ version: core.version, paths }));
await cp('art/path-overlays.json', 'web/src/generated/path-overlays.json');
for (const locale of ['pl', 'en']) {
  const source = await readFile(`target/content/i18n.${locale}.ftl`, 'utf8');
  const pathNames = new Set(paths.map((p) => `${p.key.replace('.', '-')}-name`));
  const cardKeys = new Set(
    cards.flatMap((c) => ['name', 'text'].map((field) => `${c.key.replace('.', '-')}-${field}`)),
  );
  const messages = source.split(/(?=^[a-zA-Z][\w-]*\s*=)/m);
  const names = [];
  const common = [];
  const cardMessages = [];
  for (const message of messages) {
    const key = message.match(/^([\w-]+)\s*=/)?.[1];
    (pathNames.has(key) ? names : cardKeys.has(key) ? cardMessages : common).push(message);
  }
  if (names.length !== paths.length) throw new Error(`Missing ${locale} path names`);
  await writeFile(`web/src/generated/${locale}.ftl`, common.join(''));
  await writeFile(`web/src/generated/paths.${locale}.ftl`, names.join(''));
  if (cardMessages.length !== cards.length * 2) throw new Error(`Missing ${locale} card messages`);
  await writeFile(`web/src/generated/cards.${locale}.ftl`, cardMessages.join(''));
}
// Painted illustrations, view backdrops and location scenes (AVIF, art/paintings); paintings replace the SVG drawings of the same name.
await rm('web/src/assets/paintings', { recursive: true, force: true });
const painted = {};
for (const group of ['avatars', 'cards', 'backdrops', 'scenes']) {
  await mkdir(`web/src/assets/paintings/${group}`, { recursive: true });
  const names = (await readdir(`art/paintings/${group}`)).filter((name) => name.endsWith('.avif'));
  for (const name of names) {
    await cp(`art/paintings/${group}/${name}`, `web/src/assets/paintings/${group}/${name}`);
  }
  painted[group] = new Set(names.map((name) => name.slice(0, -'.avif'.length)));
}
for (const group of ['avatars', 'icons', 'icons/online', 'frames', 'cards']) {
  await mkdir(`web/src/assets/${group}`, { recursive: true });
  for (const name of await readdir(`art/${group}`)) {
    if (!name.endsWith('.svg')) continue;
    if (painted[group]?.has(name.slice(0, -'.svg'.length))) {
      await rm(`web/src/assets/${group}/${name}`, { force: true });
      continue;
    }
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
