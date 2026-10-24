// Run after mobile sync. Signing key stays outside the repository and build artifacts.
import { createHash, createPrivateKey, createPublicKey, sign } from 'node:crypto';
import { lstat, mkdir, readdir, readFile, writeFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { crc32, deflateRawSync } from 'node:zlib';

const version = Number(process.env.WEB_BUNDLE_VERSION),
  min = Number(process.env.ANDROID_VERSION_CODE ?? 10000);
if (
  !Number.isSafeInteger(version) ||
  version < 1 ||
  version > 2147483647 ||
  !Number.isSafeInteger(min) ||
  min < 1 ||
  min > 2147483647
)
  throw new Error('Set WEB_BUNDLE_VERSION and ANDROID_VERSION_CODE');
if (!process.env.WEB_BUNDLE_SIGNING_KEY)
  throw new Error('Set WEB_BUNDLE_SIGNING_KEY to a protected Ed25519 PEM file');
const key = createPrivateKey(await readFile(process.env.WEB_BUNDLE_SIGNING_KEY));
if (key.asymmetricKeyType !== 'ed25519') throw new Error('Ed25519 is required');
const root = resolve(process.env.WEB_BUNDLE_INPUT ?? 'web/dist'),
  output = resolve(process.env.WEB_BUNDLE_OUTPUT ?? 'target/web-bundle');
const assetManifest = JSON.parse(await readFile(join(root, 'online-assets.json'), 'utf8'));
if (assetManifest.platform !== 'android')
  throw new Error('Build with pnpm --filter mobile sync first');
const content = JSON.parse(await readFile('web/src/generated/content.json', 'utf8')).version;
if (!/^[a-f0-9]{64}$/.test(content)) throw new Error('Invalid content version');
const entries = [];
async function walk(dir) {
  for (const item of (await readdir(dir)).sort()) {
    const path = join(dir, item),
      stat = await lstat(path);
    if (stat.isSymbolicLink()) throw new Error('Symlinks are forbidden');
    if (stat.isDirectory()) await walk(path);
    else if (stat.isFile()) {
      const name = path.slice(root.length + 1).replaceAll('\\', '/');
      if (
        !/^[A-Za-z0-9_./-]+\.(html|js|mjs|css|wasm|json|svg|png|jpg|webp|avif|ico|woff2|txt|webmanifest)$/.test(
          name,
        )
      )
        throw new Error(`Unsupported asset: ${name}`);
      entries.push({ name, bytes: await readFile(path) });
    }
  }
}
await walk(root);
if (entries.length > 5000 || entries.reduce((n, e) => n + e.bytes.length, 0) > 40 * 1024 * 1024)
  throw new Error('Expanded bundle exceeds budget');
const local = [],
  central = [];
let offset = 0;
for (const { name, bytes } of entries) {
  const filename = Buffer.from(name),
    compressed = deflateRawSync(bytes),
    crc = crc32(bytes),
    header = Buffer.alloc(30),
    directory = Buffer.alloc(46);
  header.writeUInt32LE(0x04034b50);
  header.writeUInt16LE(20, 4);
  header.writeUInt16LE(8, 8);
  header.writeUInt32LE(crc, 14);
  header.writeUInt32LE(compressed.length, 18);
  header.writeUInt32LE(bytes.length, 22);
  header.writeUInt16LE(filename.length, 26);
  directory.writeUInt32LE(0x02014b50);
  directory.writeUInt16LE(20, 4);
  directory.writeUInt16LE(20, 6);
  directory.writeUInt16LE(8, 10);
  directory.writeUInt32LE(crc, 16);
  directory.writeUInt32LE(compressed.length, 20);
  directory.writeUInt32LE(bytes.length, 24);
  directory.writeUInt16LE(filename.length, 28);
  directory.writeUInt32LE(offset, 42);
  local.push(header, filename, compressed);
  central.push(directory, filename);
  offset += header.length + filename.length + compressed.length;
}
const index = Buffer.concat(central),
  end = Buffer.alloc(22);
end.writeUInt32LE(0x06054b50);
end.writeUInt16LE(entries.length, 8);
end.writeUInt16LE(entries.length, 10);
end.writeUInt32LE(index.length, 12);
end.writeUInt32LE(offset, 16);
const zip = Buffer.concat([...local, index, end]);
if (zip.length > 10 * 1024 * 1024) throw new Error('Compressed bundle exceeds budget');
const hash = createHash('sha256').update(zip).digest('hex');
const payload = `SPELLGAMBIT-WEB-1\n${version}\n${min}\n1\n${content}\n${hash}\n${zip.length}\n`;
const manifest = {
  version,
  min_native_version: min,
  protocol: 1,
  content_version: content,
  sha256: hash,
  size: zip.length,
  signature: sign(null, Buffer.from(payload, 'ascii'), key).toString('base64'),
};
await mkdir(output, { recursive: true });
await writeFile(join(output, `${version}.zip`), zip);
await writeFile(join(output, 'manifest.json'), `${JSON.stringify(manifest, null, 2)}\n`);
const publicKey = createPublicKey(key)
  .export({ type: 'spki', format: 'der' })
  .subarray(-32)
  .toString('base64');
await writeFile(join(output, 'public-key.txt'), `${publicKey}\n`);
console.log(`Signed web bundle ${version}: ${zip.length} bytes, SHA-256 ${hash}`);
