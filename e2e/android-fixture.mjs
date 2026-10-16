import { execFileSync, spawn } from 'node:child_process';
import { generateKeyPairSync } from 'node:crypto';
import { createWriteStream } from 'node:fs';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { request } from 'node:http';
import { createServer } from 'node:https';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

export const fixture = resolve('target/android-fixture');
export async function prepare() {
  await mkdir(fixture, { recursive: true });
  const { privateKey, publicKey } = generateKeyPairSync('ed25519');
  await writeFile(
    `${fixture}/bundle-private.pem`,
    privateKey.export({ type: 'pkcs8', format: 'pem' }),
  );
  await writeFile(
    `${fixture}/bundle-public.txt`,
    publicKey.export({ type: 'spki', format: 'der' }).subarray(-32).toString('base64'),
  );
  await writeFile(
    `${fixture}/openssl.cnf`,
    '[req]\ndistinguished_name=dn\nx509_extensions=ext\nprompt=no\n[dn]\nCN=Spellgambit local disposable test\n[ext]\nsubjectAltName=IP:10.0.2.2,IP:127.0.0.1,DNS:localhost\nbasicConstraints=critical,CA:TRUE\nkeyUsage=critical,digitalSignature,keyEncipherment,keyCertSign\nextendedKeyUsage=serverAuth\n',
  );
  const openssl =
    process.env.OPENSSL ??
    (process.platform === 'win32' ? 'C:/Program Files/Git/usr/bin/openssl.exe' : 'openssl');
  execFileSync(
    openssl,
    [
      'req',
      '-x509',
      '-newkey',
      'rsa:2048',
      '-nodes',
      '-days',
      '2',
      '-config',
      `${fixture}/openssl.cnf`,
      '-keyout',
      `${fixture}/key.pem`,
      '-out',
      `${fixture}/cert.pem`,
    ],
    { stdio: 'ignore' },
  );
}
export async function startFixture({ bundles = false } = {}) {
  const log = createWriteStream(`${fixture}/backend.log`);
  const backend = spawn(
    'cargo',
    ['run', '--quiet', '-p', 'server', '--example', 'browser_fixture', '--locked'],
    { stdio: ['ignore', 'pipe', 'pipe'] },
  );
  backend.stdout.pipe(log);
  backend.stderr.pipe(log);
  const deadline = Date.now() + 180000;
  while (true) {
    if (backend.exitCode !== null)
      throw new Error('Isolated backend failed; inspect target/android-fixture/backend.log');
    try {
      if ((await fetch('http://127.0.0.1:18092/healthz')).ok) break;
    } catch {}
    if (Date.now() > deadline) {
      backend.kill();
      throw new Error('Isolated backend timed out');
    }
    await new Promise((r) => setTimeout(r, 250));
  }
  const server = createServer(
    { key: await readFile(`${fixture}/key.pem`), cert: await readFile(`${fixture}/cert.pem`) },
    (req, res) => {
      const asset = req.url?.match(/^\/app-bundles\/(manifest\.json|[1-9][0-9]*\.zip)$/)?.[1];
      if (asset && bundles) {
        void readFile(`${fixture}/cdn/${asset}`)
          .then((bytes) =>
            res
              .writeHead(200, {
                'Content-Type': asset.endsWith('.json') ? 'application/json' : 'application/zip',
                'Cache-Control': 'no-store',
              })
              .end(bytes),
          )
          .catch(() => res.writeHead(404).end());
        return;
      }
      if (!/^\/(api\/|healthz|readyz)/.test(req.url ?? '')) {
        res.writeHead(404).end();
        return;
      }
      const upstream = request(
        { host: '127.0.0.1', port: 18092, path: req.url, method: req.method, headers: req.headers },
        (reply) => {
          res.writeHead(reply.statusCode ?? 502, reply.headers);
          reply.pipe(res);
        },
      );
      upstream.on('error', () => res.writeHead(502).end());
      req.pipe(upstream);
    },
  );
  server.on('upgrade', (req, socket, head) => {
    if (!req.url?.startsWith('/ws?')) {
      socket.destroy();
      return;
    }
    const upstream = request({
      host: '127.0.0.1',
      port: 18092,
      path: req.url,
      method: req.method,
      headers: req.headers,
    });
    upstream.on('upgrade', (response, peer, extra) => {
      socket.write(
        `HTTP/1.1 101 Switching Protocols\r\n${Object.entries(response.headers)
          .map(([key, value]) => `${key}: ${value}\r\n`)
          .join('')}\r\n`,
      );
      if (head.length) peer.write(head);
      if (extra.length) socket.write(extra);
      peer.pipe(socket);
      socket.pipe(peer);
      socket.on('error', () => peer.destroy());
      peer.on('error', () => socket.destroy());
      socket.on('close', () => peer.destroy());
    });
    upstream.on('response', () => socket.destroy());
    upstream.on('error', () => socket.destroy());
    upstream.end();
  });
  await new Promise((resolve, reject) => {
    server.once('error', reject);
    server.listen(18443, '127.0.0.1', resolve);
  });
  return async () => {
    server.closeAllConnections();
    server.close();
    await fetch('http://127.0.0.1:18092/__shutdown', { method: 'POST' }).catch(() => {});
    if (backend.exitCode === null)
      await Promise.race([
        new Promise((r) => backend.once('exit', r)),
        new Promise((_, reject) =>
          setTimeout(() => {
            backend.kill();
            reject(new Error('Fixture cleanup timed out'));
          }, 15000),
        ),
      ]);
    log.end();
  };
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) await prepare();
