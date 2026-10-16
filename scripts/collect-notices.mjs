// Build from installed, lockfile-pinned sources; no network or absolute paths in the output.
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { readdir, readFile, realpath, stat, writeFile } from 'node:fs/promises';
import { dirname, join, relative, resolve } from 'node:path';

const records = [];
const server = process.argv.includes('--server');
for (const entry of JSON.parse(await readFile('legal/vendor/sources.json', 'utf8'))) {
  const bytes = await readFile(`legal/vendor/${entry.file}`);
  if (createHash('sha256').update(bytes).digest('hex') !== entry.sha256)
    throw new Error(`Vendored license changed: ${entry.file}`);
}
if (!(await readFile('rust-toolchain.toml', 'utf8')).includes('channel = "1.99.0"'))
  throw new Error('Review the standard library notices when changing the Rust toolchain');
const licenseName = /^(licen[cs]es?|copying|copyright|notice)([._-]|$)/i;
async function texts(directory, explicit) {
  const names = (await readdir(directory)).filter((name) => licenseName.test(name));
  if (explicit && !names.includes(explicit)) names.push(explicit);
  const result = [];
  for (const name of names.sort()) {
    const file = await realpath(join(directory, name));
    if (relative(directory, file).startsWith('..')) throw new Error('License outside package');
    const info = await stat(file);
    if (info.isDirectory()) {
      for (const child of (await readdir(file)).sort()) {
        const childFile = await realpath(join(file, child));
        if (relative(directory, childFile).startsWith('..'))
          throw new Error('License outside package');
        const childInfo = await stat(childFile);
        if (!childInfo.isFile()) continue;
        if (childInfo.size > 512 * 1024) throw new Error('Oversized license');
        result.push(await readFile(childFile, 'utf8'));
      }
      continue;
    }
    if (!info.isFile()) continue;
    if (info.size > 512 * 1024) throw new Error('Oversized license');
    result.push(await readFile(file, 'utf8'));
  }
  return result;
}
function add(name, license, content) {
  if (!content.length) throw new Error(`Missing license text: ${name}`);
  records.push({ name, license, content });
}
const seen = new Set();
async function npm(directory) {
  directory = await realpath(directory);
  if (seen.has(directory)) return;
  seen.add(directory);
  const pkg = JSON.parse(await readFile(join(directory, 'package.json'), 'utf8'));
  if (!pkg.private) {
    const content = await texts(directory);
    if (pkg.name === '@fluent/bundle') {
      if (pkg.version !== '0.19.1') throw new Error('Review Fluent license on upgrade');
      content.push(await readFile('legal/vendor/fluent-LICENSE.txt', 'utf8'));
      // Upstream source attribution, preserved even though the npm tarball omits LICENSE.
      content.push('Project Fluent Authors. https://projectfluent.org/');
    }
    add(`npm ${pkg.name}@${pkg.version}`, pkg.license, content);
  }
  for (const dependency of Object.keys(pkg.dependencies ?? {}).sort()) {
    let parent = directory;
    while (true) {
      const candidate = join(parent, 'node_modules', dependency);
      if (await stat(join(candidate, 'package.json')).catch(() => null)) {
        await npm(candidate);
        break;
      }
      const next = dirname(parent);
      if (next === parent) throw new Error(`Unresolved production dependency: ${dependency}`);
      parent = next;
    }
  }
}
if (server) {
  // Rust 1.99's target bundles musl 1.2.5 with its documented security patches.
  add('musl 1.2.5 (Rust 1.99 target)', 'MIT and notices below', [
    await readFile('legal/vendor/musl-COPYRIGHT.txt', 'utf8'),
  ]);
} else {
  await npm(resolve('web'));
  await npm(resolve('mobile'));
}
const metadata = JSON.parse(
  execFileSync(
    'cargo',
    [
      'metadata',
      '--offline',
      '--locked',
      '--format-version',
      '1',
      '--filter-platform',
      server ? 'x86_64-unknown-linux-gnu' : 'wasm32-unknown-unknown',
    ],
    { encoding: 'utf8', maxBuffer: 16 * 1024 * 1024 },
  ),
);
const nodes = new Map(metadata.resolve.nodes.map((node) => [node.id, node]));
const packages = new Map(metadata.packages.map((pkg) => [pkg.id, pkg]));
const visited = new Set();
async function rust(id) {
  if (visited.has(id)) return;
  visited.add(id);
  const pkg = packages.get(id);
  if (pkg.source)
    add(
      `Rust ${pkg.name}@${pkg.version}`,
      pkg.license,
      await texts(dirname(pkg.manifest_path), pkg.license_file),
    );
  for (const dependency of nodes.get(id).deps) {
    if (dependency.dep_kinds.some((kind) => kind.kind === null)) await rust(dependency.pkg);
  }
}
await rust(metadata.packages.find((pkg) => pkg.name === (server ? 'server' : 'engine-wasm')).id);
add(
  'Rust standard library 1.99.0',
  'MIT OR Apache-2.0',
  await Promise.all(
    ['MIT', 'APACHE'].map((name) => readFile(`legal/vendor/rust-LICENSE-${name}.txt`, 'utf8')),
  ),
);
if (!server) {
  const android = JSON.parse(await readFile('legal/android-runtime.json', 'utf8'));
  for (const record of android) add(`Android ${record.name}`, record.license, record.content);
}
records.sort((a, b) => a.name.localeCompare(b.name, 'en'));
const groups = new Map();
for (const record of records) {
  const text = record.content.join('\n\n').replaceAll('\r\n', '\n').trim();
  const hash = createHash('sha256').update(text).digest('hex');
  const group = groups.get(hash) ?? { names: [], text };
  group.names.push(`${record.name} (${record.license ?? 'see license below'})`);
  groups.set(hash, group);
}
await writeFile(
  server ? 'target/server-notices.txt' : 'web/public/legal/software.txt',
  'Spellgambit: Arcane Class — third-party software notices\n' +
    (server
      ? 'Server runtime components.\n\n'
      : 'Web/WASM and Android runtime components. Build tools are not distributed in the app.\n\n') +
    [...groups.values()]
      .map((group) => `${'='.repeat(72)}\n${group.names.join('\n')}\n\n${group.text}\n`)
      .join('\n'),
);
console.log(`Preserved licenses for ${records.length} runtime components`);
