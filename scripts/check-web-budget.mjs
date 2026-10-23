import { readdir, readFile, stat } from 'node:fs/promises';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { brotliCompressSync } from 'node:zlib';

const output = fileURLToPath(new URL('../web/dist/', import.meta.url));
const manifest = JSON.parse(await readFile(join(output, '.vite/manifest.json'), 'utf8'));
const assetGroups = JSON.parse(await readFile(join(output, 'online-assets.json'), 'utf8'));
const android = assetGroups.platform === 'android';
const onlineAssets = new Set(assetGroups.excluded.map((f) => join(output, f)));
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
const duel = graph('src/features/offline.ts');
// F11 adds optional game modes. Measure each mode separately from the account/lobby budget;
// every mode also remains excluded from the 210 KiB offline package.
const coreOnline = new Set(
  [
    'src/features/lobby/Lobby.tsx',
    'src/features/lobby/JourneyGate.tsx',
    'src/features/lobby/OnlineDuel.tsx',
    'src/features/lobby/OnlineRecords.tsx',
    'src/features/lobby/Trade.tsx',
    'src/features/account/Account.tsx',
    'src/features/store/Store.tsx',
    'src/features/clans/Clans.tsx',
    'src/features/collection/Progression.tsx',
    'src/app/pathArt.ts',
    'src/app/pathCatalog.ts',
  ]
    .flatMap((key) => [...graph(key)])
    .map((f) => join(output, f))
    .filter((f) => onlineAssets.has(f)),
);
const tournaments = new Set(
  [...graph('src/features/tournaments/Tournaments.tsx')]
    .filter((f) => !startup.has(f) && !coreOnline.has(join(output, f)))
    .map((f) => join(output, f)),
);
const expeditions = new Set(
  [...graph('src/features/expeditions/Expeditions.tsx')]
    .filter((f) => !startup.has(f) && !coreOnline.has(join(output, f)))
    .map((f) => join(output, f)),
);
const administration = new Set(
  [
    'src/features/admin/Admin.tsx',
    'src/features/moderation/Report.tsx',
    'src/features/moderation/Notices.tsx',
  ]
    .flatMap((key) => [...graph(key)])
    .filter((f) => !startup.has(f) && !coreOnline.has(join(output, f)))
    .map((f) => join(output, f)),
);
const lobbyRoute = new Set([...startup, ...graph('src/features/lobby/Lobby.tsx')]);
const recruitment = new Set(
  [
    ...new Set([
      ...graph('src/features/collection/Recruitment.tsx'),
      ...graph('src/features/collection/Advancement.tsx'),
    ]),
  ]
    .filter((f) => onlineAssets.has(join(output, f)) && !coreOnline.has(join(output, f)))
    .map((f) => join(output, f)),
);
const privacy = new Set(
  ['src/features/privacy/Privacy.tsx', 'src/features/legal/Legal.tsx']
    .flatMap((key) => [...graph(key)])
    .filter((f) => !startup.has(f) && !coreOnline.has(join(output, f)))
    .map((f) => join(output, f)),
);
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
// Painted illustrations (AVIF, art/paintings) are already compressed; budget per file and in total.
// The lobby's town fills the whole map, so it alone has a larger cap (docs/13-wydajnosc.md).
const paintings = files.filter((f) => f.endsWith('.avif'));
const paintingSizes = await Promise.all(paintings.map(async (f) => (await stat(f)).size));
const isTown = (f) => /[\\/]town-[\w-]+\.avif$/.test(f);
const townSize = paintingSizes.find((_, i) => isTown(paintings[i]));
// A renamed or missing town would otherwise pass its own cap unmeasured.
if (townSize === undefined)
  throw new Error('The town painting (town-*.avif) is missing from the build.');
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
    // The duel board is shared with online duels (~0.7 KiB); raised from 40 KiB by the owner.
    41,
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
  ['Painted illustrations (AVIF, total)', paintingSizes.reduce((a, b) => a + b, 0), 6144],
  [
    'Largest painted illustration except the town (AVIF)',
    Math.max(0, ...paintingSizes.filter((_, i) => !isTown(paintings[i]))),
    32,
  ],
  ['Town painting (AVIF)', townSize, 64],
  // Account progression, tutorial and level gates add ~1 KiB over the former 53 KiB cap.
  ['Online account and lobby on demand (Brotli)', await sum([...coreOnline]), 56],
  ['Tournaments on demand (Brotli)', await sum([...tournaments]), 20],
  ['Recruitment on demand (Brotli)', await sum([...recruitment]), 10],
  ['Expeditions on demand (Brotli)', await sum([...expeditions]), 20],
  ['Administration and moderation on demand (Brotli)', await sum([...administration]), 20],
  ['Privacy and legal on demand (Brotli)', await sum([...privacy]), 20],
  [
    'Software notices on demand (Brotli)',
    await sum(
      [...graph('src/features/legal/software.ts')]
        .filter((f) => !startup.has(f))
        .map((f) => join(output, f)),
    ),
    20,
  ],
  [
    'Complete initial lobby route JS (Brotli)',
    await sum([...lobbyRoute].filter((f) => f.endsWith('.js')).map((f) => join(output, f))),
    70,
  ],
  ...(!android
    ? [
        [
          'Complete offline PWA without card illustrations (Brotli, images/fonts original)',
          await sum(
            files.filter(
              (f) =>
                !illustrations.has(f) &&
                !onlineAssets.has(f) &&
                !f.endsWith('online-assets.json') &&
                !f.endsWith('.avif'),
            ),
            async (f) => (/\.(woff2|png)$/.test(f) ? (await stat(f)).size : compressed(f)),
          ),
          // Illustrated deck editing, preview and sample hands remain available offline.
          210,
        ],
      ]
    : [
        [
          // APKs contain every file, including online screens and native integrations.
          // This uncompressed asset cap is separate from the 210 KiB browser PWA cap.
          'All Android web assets including illustrations and native integrations (uncompressed)',
          await sum(files, async (f) => (await stat(f)).size),
          10 * 1024,
        ],
      ]),
];
let failed = false;
for (const [label, bytes, limit] of rows) {
  console.log(`${label}: ${(bytes / 1024).toFixed(3)} / ${limit} KiB`);
  if (bytes > limit * 1024) failed = true;
}
if (failed) process.exitCode = 1;
