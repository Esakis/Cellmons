import { execFileSync } from 'node:child_process';
import { createHash, randomBytes, X509Certificate } from 'node:crypto';
import { createReadStream } from 'node:fs';
import { mkdir, readFile, stat, writeFile } from 'node:fs/promises';
import { createServer as httpServer, request } from 'node:http';
import { get, createServer as httpsServer } from 'node:https';
import { isIP } from 'node:net';
import { resolve } from 'node:path';

// This helper exposes only the game API and a single APK, never the repository or mail.
const directory = resolve('target/phone-test');
const configuration = `${directory}/connection.json`;
const certificate = `${directory}/cert.pem`;
const privateKey = `${directory}/key.pem`;
const apk = `${directory}/Spellgambit-test.apk`;
const credentials = `${directory}/konto-testowe.json`;
const upstreamPort = 8090;
const headers = {
  'Content-Type': 'application/json',
  'X-Requested-With': 'app',
  'X-Native-Version': '1.0.1',
  Origin: 'https://localhost',
};

function localAddress(value) {
  if (isIP(value) !== 4) return false;
  const [a, b] = value.split('.').map(Number);
  return a === 10 || a === 127 || (a === 172 && b >= 16 && b <= 31) || (a === 192 && b === 168);
}

async function config() {
  return JSON.parse(await readFile(configuration, 'utf8'));
}

async function prepare(address) {
  if (!localAddress(address)) throw new Error('Use a private IPv4 address of this computer');
  await mkdir(directory, { recursive: true });
  let reuse = false;
  try {
    const cert = new X509Certificate(await readFile(certificate));
    reuse = cert.checkIP(address) === address && Date.parse(cert.validTo) > Date.now() + 86400000;
    await stat(privateKey);
  } catch {}
  if (!reuse) {
    const openssl =
      process.env.OPENSSL ??
      (process.platform === 'win32' ? 'C:/Program Files/Git/usr/bin/openssl.exe' : 'openssl');
    const san = [...new Set([address, '127.0.0.1', '10.0.2.2'])].map((ip) => `IP:${ip}`);
    await writeFile(
      `${directory}/openssl.cnf`,
      `[req]
distinguished_name=dn
x509_extensions=ext
prompt=no
[dn]
CN=Spellgambit private phone test
[ext]
subjectAltName=${san.join(',')},DNS:localhost
basicConstraints=critical,CA:TRUE
keyUsage=critical,digitalSignature,keyEncipherment,keyCertSign
extendedKeyUsage=serverAuth
`,
    );
    execFileSync(
      openssl,
      [
        'req',
        '-x509',
        '-newkey',
        'rsa:2048',
        '-nodes',
        '-days',
        '90',
        '-config',
        `${directory}/openssl.cnf`,
        '-keyout',
        privateKey,
        '-out',
        certificate,
      ],
      { stdio: 'ignore' },
    );
  }
  const cert = new X509Certificate(await readFile(certificate));
  const value = {
    address,
    apiOrigin: `https://${address}:18443`,
    downloadOrigin: `http://${address}:18080`,
    packageId: 'dev.game.training.phone',
    certificateSha256: cert.fingerprint256,
    certificateExpires: cert.validTo,
  };
  await writeFile(configuration, `${JSON.stringify(value, null, 2)}\n`);
  console.log(`Phone API: ${value.apiOrigin}`);
}

function allowed(req) {
  const remote = req.socket.remoteAddress?.replace(/^::ffff:/, '');
  return remote === '::1' || (remote && localAddress(remote));
}

async function serve() {
  const value = await config();
  const sockets = new Set();
  const api = httpsServer(
    { key: await readFile(privateKey), cert: await readFile(certificate), minVersion: 'TLSv1.2' },
    (req, res) => {
      if (!allowed(req) || !/^\/(api\/|healthz$|readyz$)/.test(req.url ?? '')) {
        res.writeHead(404).end();
        return;
      }
      const peer = request(
        {
          host: '127.0.0.1',
          port: upstreamPort,
          path: req.url,
          method: req.method,
          headers: req.headers,
        },
        (reply) => {
          res.writeHead(reply.statusCode ?? 502, reply.headers);
          reply.pipe(res);
        },
      );
      peer.on('error', () => {
        if (!res.headersSent) res.writeHead(502);
        res.end();
      });
      req.on('aborted', () => peer.destroy());
      peer.setTimeout(30000, () => peer.destroy());
      req.pipe(peer);
    },
  );
  api.on('upgrade', (req, socket, head) => {
    if (!allowed(req) || !req.url?.startsWith('/ws?')) {
      socket.destroy();
      return;
    }
    const peer = request({
      host: '127.0.0.1',
      port: upstreamPort,
      path: req.url,
      method: req.method,
      headers: req.headers,
    });
    peer.on('upgrade', (reply, upstream, extra) => {
      socket.write(
        `HTTP/1.1 101 Switching Protocols\r\n${Object.entries(reply.headers)
          .map(([key, val]) => `${key}: ${val}\r\n`)
          .join('')}\r\n`,
      );
      if (head.length) upstream.write(head);
      if (extra.length) socket.write(extra);
      upstream.pipe(socket);
      socket.pipe(upstream);
      socket.on('error', () => upstream.destroy());
      upstream.on('error', () => socket.destroy());
      socket.on('close', () => upstream.destroy());
      upstream.on('close', () => socket.destroy());
    });
    peer.on('response', () => socket.destroy());
    peer.on('error', () => socket.destroy());
    peer.end();
  });
  const download = httpServer(async (req, res) => {
    res.setHeader('Cache-Control', 'no-store');
    res.setHeader('X-Content-Type-Options', 'nosniff');
    res.setHeader('Content-Security-Policy', "default-src 'none'; style-src 'unsafe-inline'");
    if (!allowed(req) || req.method !== 'GET') {
      res.writeHead(404).end();
      return;
    }
    if (req.url === '/Spellgambit-test.apk') {
      try {
        const file = await stat(apk);
        res.writeHead(200, {
          'Content-Type': 'application/vnd.android.package-archive',
          'Content-Disposition': 'attachment; filename="Spellgambit-test.apk"',
          'Content-Length': file.size,
        });
        createReadStream(apk)
          .on('error', () => res.destroy())
          .pipe(res);
      } catch {
        res.writeHead(503).end('APK jest jeszcze przygotowywany.');
      }
      return;
    }
    if (req.url !== '/') {
      res.writeHead(404).end();
      return;
    }
    res.writeHead(200, { 'Content-Type': 'text/html; charset=utf-8' }).end(`<!doctype html>
<html lang="pl"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Spellgambit — test na telefonie</title>
<style>body{font:18px/1.6 system-ui;max-width:620px;margin:40px auto;padding:0 22px;background:#141823;color:#f5efe1}h1{line-height:1.2}a{display:block;background:#e6bf73;color:#171a24;padding:16px;text-align:center;border-radius:12px;font-weight:bold;text-decoration:none}li{margin:12px 0}small{color:#bac2cf}</style>
<h1>Spellgambit na telefonie</h1><p>Wersja do testów w domowej sieci.</p>
<a href="/Spellgambit-test.apk">Pobierz APK</a>
<ol><li>Zainstaluj pobrany plik. Jeśli Android zapyta, zezwól tej przeglądarce na instalację aplikacji.</li>
<li>Otwórz aplikację i zaloguj się danymi z pliku <b>konto-testowe.json</b> na komputerze.</li>
<li>Wejdź do Lobby → Arena, wybierz wolnego gracza i wyślij wyzwanie.</li></ol>
<p>Komputer i Docker muszą pozostać włączone. Telefon musi mieć dostęp do tej samej sieci lokalnej.</p>
<small>Po restarcie komputera uruchom phone.bat. Wersja testowa instaluje się osobno od wcześniejszego APK.</small></html>`);
  });
  for (const server of [api, download]) {
    server.on('connection', (socket) => {
      sockets.add(socket);
      socket.on('close', () => sockets.delete(socket));
    });
  }
  const listen = (server, port) =>
    new Promise((done, reject) => {
      server.once('error', reject);
      server.listen(port, '0.0.0.0', done);
    });
  await listen(api, 18443);
  await listen(download, 18080);
  console.log(`Ready. Download: ${value.downloadOrigin}`);
  const stop = () => {
    for (const socket of sockets) socket.destroy();
    api.close();
    download.close();
  };
  process.once('SIGINT', stop);
  process.once('SIGTERM', stop);
}

async function post(path, body) {
  const response = await fetch(`http://127.0.0.1:${upstreamPort}${path}`, {
    method: 'POST',
    headers,
    body: JSON.stringify(body),
  });
  if (!response.ok) throw new Error(`${path}: HTTP ${response.status}`);
  return response;
}

async function account() {
  let existing;
  try {
    existing = JSON.parse(await readFile(credentials, 'utf8'));
  } catch {}
  if (existing) {
    if (existing.verified) {
      console.log('Existing phone test account preserved.');
      return;
    }
    const response = await post('/api/auth/login', {
      identity: existing.login,
      password: existing.password,
      client: 'android',
    });
    const session = await response.json();
    if (!session.profile.email_verified) await verifyEmail(existing.email);
    await fetch(`http://127.0.0.1:${upstreamPort}/api/auth/logout`, {
      method: 'POST',
      headers: { ...headers, Authorization: `Bearer ${session.token}` },
    });
    await writeFile(credentials, `${JSON.stringify({ ...existing, verified: true }, null, 2)}\n`, {
      mode: 0o600,
    });
    console.log('Existing phone test account is ready.');
    return;
  }
  const login = `Telefon_${randomBytes(3).toString('hex')}`;
  const password = `${randomBytes(18).toString('base64url')}!Aa7`;
  const email = `${login.toLowerCase()}@phone-test.invalid`;
  const legal = JSON.parse(await readFile('legal/documents.json', 'utf8'));
  await post('/api/auth/register', {
    login,
    email,
    password,
    school: 'arcane',
    terms_version: legal.version,
    age_confirmed: true,
    client: 'android',
  });
  // Persist before verification so a lost response never replaces an existing account.
  await writeFile(credentials, `${JSON.stringify({ login, password, email }, null, 2)}\n`, {
    mode: 0o600,
  });
  await verifyEmail(email);
  await writeFile(
    credentials,
    `${JSON.stringify({ login, password, email, verified: true }, null, 2)}\n`,
    { mode: 0o600 },
  );
  console.log('Phone test account created. Credentials: target/phone-test/konto-testowe.json');
}

async function verifyEmail(email) {
  const { readdir } = await import('node:fs/promises');
  const mailDirectory = resolve('target/mail');
  for (let retry = 0; retry < 40; retry++) {
    for (const name of await readdir(mailDirectory).catch(() => [])) {
      if (!name.endsWith('.json')) continue;
      let mail;
      try {
        mail = JSON.parse(await readFile(`${mailDirectory}/${name}`, 'utf8'));
      } catch {
        continue;
      }
      if (mail.to !== email) continue;
      const match = mail.body?.match(/https?:\/\/\S+/);
      if (!match) continue;
      const token = new URLSearchParams(match[0].split('?')[1]).get('token');
      if (token) {
        await post('/api/auth/verify-email', { token });
        return;
      }
    }
    await new Promise((done) => setTimeout(done, 500));
  }
  throw new Error('Verification mail missing; inspect the local target/mail outbox');
}

async function check() {
  const value = await config();
  const ca = await readFile(certificate);
  await new Promise((done, reject) => {
    get(`${value.apiOrigin}/readyz`, { ca, timeout: 5000 }, (response) => {
      response.resume();
      if (response.statusCode === 200) done();
      else reject(new Error(`Readiness: HTTP ${response.statusCode}`));
    })
      .on('error', reject)
      .on('timeout', function () {
        this.destroy(new Error('TLS timeout'));
      });
  });
  const response = await fetch(`${value.downloadOrigin}/Spellgambit-test.apk`);
  if (!response.ok) throw new Error(`APK download: HTTP ${response.status}`);
  const bytes = Buffer.from(await response.arrayBuffer());
  const expected = await readFile(apk);
  if (!bytes.equals(expected)) throw new Error('Downloaded APK differs from the built artifact');
  console.log(`Verified TLS, database readiness and APK download (${bytes.length} bytes).`);
  console.log(`SHA-256: ${createHash('sha256').update(bytes).digest('hex')}`);
}

switch (process.argv[2]) {
  case 'prepare':
    await prepare(process.argv[3]);
    break;
  case 'serve':
    await serve();
    break;
  case 'account':
    await account();
    break;
  case 'check':
    await check();
    break;
  default:
    throw new Error('Usage: phone-gateway.mjs prepare ADDRESS | serve | account | check');
}
