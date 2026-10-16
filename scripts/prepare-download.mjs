// Creates a reviewable static APK download artifact; never publishes it.
import { createHash } from 'node:crypto';
import { copyFile, mkdir, readFile, stat, writeFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { inflateRawSync } from 'node:zlib';

const [apkPath, outputPath = 'target/android-release/downloads'] = process.argv.slice(2);
const version = process.env.ANDROID_VERSION;
const applicationId = process.env.ANDROID_APPLICATION_ID;
const release = process.env.GAME_RELEASE;
if (
  !apkPath ||
  !/^\d{1,3}\.\d{1,2}\.\d{1,2}$/.test(version ?? '') ||
  !/^[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*){2,}$/.test(applicationId ?? '') ||
  !/^[a-f0-9]{40}$/.test(release ?? '')
)
  throw new Error(
    'APK path, ANDROID_VERSION, ANDROID_APPLICATION_ID and full GAME_RELEASE are required',
  );
const size = (await stat(apkPath)).size;
if (size < 1024 || size > 20 * 1024 * 1024) throw new Error('APK exceeds the release size bounds');
const bundle = JSON.parse(await readFile('target/web-bundle/manifest.json', 'utf8'));
const meta = JSON.parse(await readFile('web/dist/.vite/release.json', 'utf8'));
if (
  meta.release !== release ||
  meta.content !== bundle.content_version ||
  !/^[a-f0-9]{64}$/.test(meta.content) ||
  typeof meta.engine !== 'string' ||
  bundle.version !== Number(process.env.WEB_BUNDLE_VERSION)
)
  throw new Error('APK web build, signed bundle and release metadata differ');
const bytes = await readFile(apkPath);
// Read the actual build metadata inside the signed APK, not a possibly stale
// neighbouring dist directory. APK v2/v3 signing blocks precede the ZIP directory.
function apkEntry(name) {
  let end = bytes.length - 22;
  for (; end >= Math.max(0, bytes.length - 65557); end--)
    if (
      bytes.readUInt32LE(end) === 0x06054b50 &&
      end + 22 + bytes.readUInt16LE(end + 20) === bytes.length
    )
      break;
  if (end < 0 || bytes.readUInt32LE(end) !== 0x06054b50)
    throw new Error('Invalid APK ZIP directory');
  const count = bytes.readUInt16LE(end + 10);
  let offset = bytes.readUInt32LE(end + 16);
  if (count > 10000) throw new Error('APK ZIP entry limit');
  for (let i = 0; i < count; i++) {
    if (offset + 46 > end || bytes.readUInt32LE(offset) !== 0x02014b50)
      throw new Error('Invalid APK entry');
    const nameLength = bytes.readUInt16LE(offset + 28);
    const filename = bytes.subarray(offset + 46, offset + 46 + nameLength).toString('utf8');
    if (filename === name) {
      const size = bytes.readUInt32LE(offset + 24),
        compressed = bytes.readUInt32LE(offset + 20);
      const local = bytes.readUInt32LE(offset + 42),
        method = bytes.readUInt16LE(offset + 10);
      if (
        size > 8192 ||
        compressed > 8192 ||
        local + 30 > bytes.length ||
        bytes.readUInt32LE(local) !== 0x04034b50
      )
        throw new Error('Invalid APK metadata size');
      const start = local + 30 + bytes.readUInt16LE(local + 26) + bytes.readUInt16LE(local + 28);
      if (start + compressed > offset) throw new Error('Invalid APK metadata offset');
      const raw = bytes.subarray(start, start + compressed);
      const decoded =
        method === 0 ? raw : method === 8 ? inflateRawSync(raw, { maxOutputLength: 8192 }) : null;
      if (!decoded || decoded.length !== size) throw new Error('Invalid APK metadata compression');
      return JSON.parse(decoded.toString('utf8'));
    }
    offset += 46 + nameLength + bytes.readUInt16LE(offset + 30) + bytes.readUInt16LE(offset + 32);
  }
  throw new Error(`APK is missing ${name}`);
}
const embedded = apkEntry('assets/public/native-release.json');
const capacitor = apkEntry('assets/capacitor.config.json');
if (
  ['release', 'engine', 'content'].some((key) => embedded[key] !== meta[key]) ||
  capacitor.appId !== applicationId ||
  embedded.application_id !== applicationId ||
  embedded.version !== version ||
  embedded.web_bundle_version !== bundle.version
)
  throw new Error('Signed APK belongs to a different application or web release');
const sha256 = createHash('sha256').update(bytes).digest('hex');
const filename = `spellgambit-${version}-${sha256.slice(0, 16)}.apk`;
const output = resolve(outputPath);
await mkdir(output, { recursive: true });
await copyFile(apkPath, join(output, filename));
const manifest = {
  version,
  application_id: applicationId,
  release,
  engine: meta.engine,
  content: meta.content,
  web_bundle_version: bundle.version,
  filename,
  size,
  sha256,
};
await writeFile(join(output, 'release.json'), `${JSON.stringify(manifest, null, 2)}\n`);
await writeFile(join(output, 'SHA256SUMS'), `${sha256}  ${filename}\n`);
const html = `<!doctype html><html lang="pl"><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>Spellgambit — Android APK</title><link rel="stylesheet" href="style.css"><main>
<h1>Spellgambit — Android</h1><p>Wersja / Version: ${version}</p>
<p><a class="download" href="${filename}" download>Pobierz APK / Download APK</a> (${(size / 1048576).toFixed(1)} MiB)</p>
<p>Wymagany Android 8.0 lub nowszy. Pobieraj aplikację wyłącznie z oficjalnej strony wydawcy. Instalacja spoza sklepu wymaga zgody Androida dla używanej przeglądarki. Późniejsze aktualizacje APK wymagają tego samego podpisu wydawcy.</p>
<p lang="en">Requires Android 8.0 or later. Download only from the publisher's official site. Android requires permission for this browser to install apps. APK updates must use the same publisher signing key.</p>
<h2>SHA-256</h2><pre>${sha256}</pre><p><a href="SHA256SUMS">Plik sum kontrolnych / Checksum file</a> · <a href="release.json">Dane wydania / Release metadata</a></p>
<p>Gość gra lokalnie bez zapisu postępu. Konto online wymaga rejestracji.</p><p lang="en">Guest play is local and does not save progress. Online accounts require registration.</p>
<nav><a href="/">Graj / Play</a> · <a href="/legal/privacy.html">Prywatność / Privacy</a> · <a href="/legal/deletion.html">Usuń konto / Delete account</a> · <a href="/legal/terms.html">Regulamin / Terms</a></nav>
</main></html>\n`;
await writeFile(join(output, 'index.html'), html);
await writeFile(
  join(output, 'style.css'),
  'body{font:1rem/1.6 system-ui,sans-serif;background:#131c28;color:#f1f5f9;margin:0}main{max-width:48rem;margin:auto;padding:1.5rem}a{color:#8fd5ff}a:focus-visible{outline:3px solid #f4cd70;outline-offset:4px}.download{display:inline-block;padding:.7rem 1rem;border:2px solid;border-radius:.3rem}pre{white-space:pre-wrap;overflow-wrap:anywhere}nav{margin-top:2rem}\n',
);
console.log(`Prepared APK download artifact: ${filename}, SHA-256 ${sha256}`);
