// Run after :app:dependencyInventory. Commit the reviewed result; CI rejects stale notices.
import { readdir, readFile, writeFile } from 'node:fs/promises';
import { homedir } from 'node:os';
import { join } from 'node:path';

const raw = JSON.parse(await readFile('target/android-runtime-licenses.json', 'utf8'));
const cache = join(
  process.env.GRADLE_USER_HOME ?? join(homedir(), '.gradle'),
  'caches/modules-2/files-2.1',
);
const apache = await readFile('legal/vendor/rust-LICENSE-APACHE.txt', 'utf8');
const records = new Map();
async function publishedLicense(coordinate, depth = 0) {
  if (depth > 8 || !/^[\w.-]+:[\w.-]+:[\w.+-]+$/.test(coordinate))
    throw new Error('Invalid POM parent');
  const directory = join(cache, ...coordinate.split(':'));
  let pom;
  for (const hash of await readdir(directory)) {
    for (const name of await readdir(join(directory, hash))) {
      if (name.endsWith('.pom')) pom = await readFile(join(directory, hash, name), 'utf8');
    }
  }
  if (!pom) throw new Error(`Missing published POM: ${coordinate}`);
  const license = pom.match(/<licenses>[\s\S]*?<name>([\s\S]*?)<\/name>/)?.[1]?.trim();
  if (license) return license;
  const parent = pom.match(/<parent>([\s\S]*?)<\/parent>/)?.[1];
  if (!parent) return undefined;
  const values = ['groupId', 'artifactId', 'version'].map(
    (field) => parent.match(new RegExp(`<${field}>([^<]+)</${field}>`))?.[1],
  );
  if (values.some((value) => !value)) throw new Error('Unresolved POM parent');
  return publishedLicense(values.join(':'), depth + 1);
}
for (const record of raw) {
  if (records.has(record.name)) continue;
  const license = await publishedLicense(record.name);
  if (!record.content.length) {
    if (!/Apache.*2\.0|Apache.*Version 2/i.test(license ?? ''))
      throw new Error(`Review missing license: ${record.name} (${license})`);
    record.content.push(apache);
  }
  records.set(record.name, {
    name: record.name,
    license: license ?? 'See included license',
    content: record.content,
  });
}
const output = `${JSON.stringify(
  [...records.values()].sort((a, b) => a.name.localeCompare(b.name, 'en')),
  null,
  2,
)}\n`;
const path = 'legal/android-runtime.json';
if (process.argv.includes('--check')) {
  if ((await readFile(path, 'utf8')) !== output)
    throw new Error(
      'Android runtime notices changed; regenerate and review legal/android-runtime.json',
    );
} else await writeFile(path, output);
console.log(`Verified notices for ${records.size} Maven runtime components`);
