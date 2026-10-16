// Real production Caddyfile, task-local CA and synthetic backend. No TLS verification bypass.
import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { randomBytes } from 'node:crypto';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import https from 'node:https';
import { resolve } from 'node:path';
import { setTimeout as sleep } from 'node:timers/promises';
import { promisify } from 'node:util';

const exec = promisify(execFile);
const run = async (command, args) =>
  (
    await exec(command, args, { windowsHide: true, timeout: 120000, maxBuffer: 8 * 1024 * 1024 })
  ).stdout.trim();
const docker = (...args) => run('docker', args);
const id = `game-proxy-${randomBytes(6).toString('hex')}`;
const root = resolve('target', id),
  config = resolve(root, 'private'),
  web = resolve(root, 'web');
await mkdir(resolve(config), { recursive: true });
await mkdir(resolve(web, 'current/.vite'), { recursive: true });
await mkdir(resolve(web, 'assets'), { recursive: true });
await mkdir(resolve(web, 'downloads'), { recursive: true });
await writeFile(resolve(web, 'downloads/index.html'), 'download-fixture');
await writeFile(resolve(web, 'current/index.html'), 'fixture-home');
await writeFile(resolve(web, 'current/.env'), 'private-fixture-env');
await writeFile(resolve(web, 'current/.vite/manifest.json'), 'private-fixture-manifest');
await writeFile(resolve(web, 'assets/retained.js'), 'old-client-fixture');
const openssl =
  process.env.OPENSSL ??
  (process.platform === 'win32' ? 'C:/Program Files/Git/usr/bin/openssl.exe' : 'openssl');
await run(openssl, [
  'req',
  '-x509',
  '-newkey',
  'rsa:2048',
  '-nodes',
  '-days',
  '1',
  '-subj',
  '/CN=localhost',
  '-addext',
  'subjectAltName=DNS:localhost,IP:127.0.0.1',
  '-keyout',
  resolve(config, 'origin-key.pem'),
  '-out',
  resolve(config, 'origin.pem'),
]);
await writeFile(
  resolve(config, 'trusted-proxies.caddy'),
  'trusted_proxies static 172.16.0.0/12 127.0.0.0/8 192.168.0.0/16\n',
);
await writeFile(
  resolve(config, 'active.caddy'),
  `reverse_proxy ${id}-backend:8080 {\n header_up X-Forwarded-For {client_ip}\n header_up -Forwarded\n stream_close_delay 10m\n}\n`,
);
const ca = await readFile(resolve(config, 'origin.pem'));
let port;
async function get(path, headers = {}) {
  return new Promise((resolveGet, reject) => {
    const request = https.get(
      {
        hostname: '127.0.0.1',
        port,
        path,
        servername: 'localhost',
        ca,
        headers: { Host: 'localhost', ...headers },
        timeout: 5000,
      },
      (response) => {
        let body = '';
        response.on('data', (chunk) => {
          body += chunk;
          if (body.length > 1024 * 1024) request.destroy(new Error('Oversized fixture response'));
        });
        response.on('end', () =>
          resolveGet({ status: response.statusCode, headers: response.headers, body }),
        );
      },
    );
    request.on('timeout', () => request.destroy(new Error('Proxy response timeout')));
    request.on('error', reject);
  });
}
try {
  await docker('network', 'create', id);
  await docker('pull', 'node:24.19.0-bookworm-slim');
  await docker(
    'run',
    '-d',
    '--name',
    `${id}-backend`,
    '--network',
    id,
    'node:24.19.0-bookworm-slim',
    'node',
    '-e',
    "require('http').createServer((req,res)=>{res.setHeader('Content-Type','application/json');res.end(JSON.stringify({forwarded:req.headers['x-forwarded-for']}));}).listen(8080,'0.0.0.0')",
  );
  await docker(
    'run',
    '-d',
    '--name',
    `${id}-caddy`,
    '--network',
    id,
    '-p',
    '127.0.0.1::8443',
    '-e',
    'GAME_DOMAIN=https://localhost:8443',
    '-v',
    `${resolve('deploy/Caddyfile')}:/etc/caddy/Caddyfile:ro`,
    '-v',
    `${config}:/etc/caddy/private:ro`,
    '-v',
    `${web}:/srv/web:ro`,
    'caddy:2.11.7-alpine@sha256:d8542f48d34a9cf4e4c11a478865229840e87e4c96ea3f439101f31a5d35f75f',
  );
  const published = await docker('port', `${id}-caddy`, '8443');
  assert.match(published, /^127\.0\.0\.1:\d+$/);
  port = Number(published.split(':')[1]);
  let ready = false;
  for (let i = 0; i < 40; i++) {
    if (
      await get('/').then(
        (r) => r.status === 200,
        () => false,
      )
    ) {
      ready = true;
      break;
    }
    await sleep(250);
  }
  assert(ready, 'Proxy did not become ready');
  for (const path of [
    '/metrics',
    '/readyz',
    '/.env',
    '/.git/config',
    '/.vite/manifest.json',
    '/admin/drain',
  ])
    assert.equal((await get(path)).status, 404, path);
  const old = await get('/assets/retained.js');
  assert.equal(old.body, 'old-client-fixture');
  assert.match(old.headers['cache-control'], /immutable/);
  const page = await get('/another-client-route');
  assert.equal(page.body, 'fixture-home');
  assert.match(page.headers['content-security-policy'], /frame-ancestors 'none'/);
  assert.equal((await get('/downloads/')).body, 'download-fixture');
  assert.equal((await get('/downloads/missing.apk')).status, 404);
  const api = await get('/api/echo', {
    'CF-Connecting-IP': '198.51.100.77',
    'X-Forwarded-For': '203.0.113.66',
  });
  assert.equal(api.status, 200);
  assert.equal(JSON.parse(api.body).forwarded, '198.51.100.77');
  assert.match(api.headers['cache-control'], /no-store/);
  await docker('stop', `${id}-backend`);
  assert.equal(
    (
      await get('/api/offline?ticket=fixture-token', {
        Authorization: 'Bearer fixture-secret',
        Cookie: 'fixture-cookie',
      })
    ).status,
    502,
  );
  const logs = await exec('docker', ['logs', `${id}-caddy`], { windowsHide: true });
  assert(!`${logs.stdout}${logs.stderr}`.includes('fixture-token'));
  assert(!`${logs.stdout}${logs.stderr}`.includes('fixture-secret'));
  assert(!`${logs.stdout}${logs.stderr}`.includes('fixture-cookie'));
  await writeFile(
    'target/f14-proxy-proof.json',
    JSON.stringify(
      {
        success: true,
        synthetic: true,
        private_paths: 6,
        strict_tls: true,
        retained_chunks: true,
        client_ip_overwrite: true,
        credential_logs_redacted: true,
      },
      null,
      2,
    ),
  );
  console.log(
    'Production proxy: strict TLS, private paths, retained chunks, client IP and credential redaction passed',
  );
} finally {
  for (const name of [`${id}-caddy`, `${id}-backend`])
    await docker('rm', '-f', name).catch(() => {});
  await docker('network', 'rm', id).catch(() => {});
}
