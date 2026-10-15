import { readdir, readFile, stat } from 'node:fs/promises';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { brotliCompressSync } from 'node:zlib';

const output = fileURLToPath(new URL('../web/dist/', import.meta.url));
const manifest = JSON.parse(await readFile(join(output, '.vite/manifest.json'), 'utf8'));
const onlineAssets = new Set(
  JSON.parse(await readFile(join(output, 'online-assets.json'), 'utf8')).excluded.map((f) =>
    join(output, f),
  ),
);
function graph(key, files = new Set()) {
  const entry = manifest[key];
  if (!entry || files.has(entry.file)) return files;
  files.add(entry.file);
  for (const css of entry.css ?? []) files.add(css);
  for (const imported of entry.imports ?? []) graph(imported, files);
  return files;
}
const startup = new Set();
for (const [key, entry] of Object.entries(manifest)) if (entry.isEntry) graph(key, startup);
const duel = graph('src/features/training/Training.tsx');
// Card illustrations have their own budget (docs/13-wydajnosc.md, rozdz. 1.1).
const illustrations = new Set(
  [manifest['src/app/cardArt.ts']?.file].filter(Boolean).map((f) => join(output, f)),
);
async function walk(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const paths = await Promise.all(
    entries
      .filter((e) => !e.name.startsWith('.'))
      .map((e) => (e.isDirectory() ? walk(join(directory, e.name)) : [join(directory, e.name)])),
  );
  return paths.flat();
}
const files = await walk(output);
const compressed = async (file) => brotliCompressSync(await readFile(file)).length;
async function sum(list, size = compressed) {
  return (await Promise.all(list.map(size))).reduce((a, b) => a + b, 0);
}
const rows = [
  [
    'Startup JS (Brotli)',
    await sum([...startup].filter((f) => f.endsWith('.js')).map((f) => join(output, f))),
    70,
  ],
  [
    'Startup CSS (Brotli)',
    await sum([...startup].filter((f) => f.endsWith('.css')).map((f) => join(output, f))),
    20,
  ],
  [
    'Duel JS (Brotli)',
    await sum(
      [...duel].filter((f) => f.endsWith('.js') && !startup.has(f)).map((f) => join(output, f)),
    ),
    40,
  ],
  [
    'Fonts (WOFF2)',
    await sum(
      files.filter((f) => f.endsWith('.woff2')),
      async (f) => (await stat(f)).size,
    ),
    40,
  ],
  ['WASM (Brotli)', await sum(files.filter((f) => f.endsWith('.wasm'))), 90],
  ['Card illustrations (Brotli)', await sum([...illustrations]), 80],
  ['Online account and lobby on demand (Brotli)', await sum([...onlineAssets]), 45],
  [
    'Complete offline app without card illustrations (Brotli, images/fonts original)',
    await sum(
      files.filter(
        (f) => !illustrations.has(f) && !onlineAssets.has(f) && !f.endsWith('online-assets.json'),
      ),
      async (f) => (/\.(woff2|png)$/.test(f) ? (await stat(f)).size : compressed(f)),
    ),
    200,
  ],
];
let failed = false;
for (const [label, bytes, limit] of rows) {
  console.log(`${label}: ${(bytes / 1024).toFixed(1)} / ${limit} KiB`);
  if (bytes > limit * 1024) failed = true;
}
if (failed) process.exitCode = 1;
