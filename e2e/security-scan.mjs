import { spawn } from 'node:child_process';
import { chmod, mkdir, open, readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';

// No supplied targets or credentials: scans only our disposable loopback fixture and built client.
const image =
  'ghcr.io/zaproxy/zaproxy@sha256:7aaa659b0d43078febd82e29bad112285c370727e86ab8340444220e17d9f0d2';
const output = resolve('target/zap');
await mkdir(output, { recursive: true });
await chmod(output, 0o777);
const container = `spellgambit-zap-${process.pid}`;
const children = [];
const logs = [];
async function start(command, args, name, env = process.env, cwd = process.cwd()) {
  const file = await open(resolve(output, `${name}.log`), 'w');
  logs.push(file);
  const child = spawn(command, args, {
    env,
    cwd,
    windowsHide: true,
    stdio: ['ignore', file.fd, file.fd],
  });
  children.push(child);
  child.on('error', () => {});
  return child;
}
async function waitURL(url) {
  for (let i = 0; i < 180; i++) {
    if (children.some((child) => child.exitCode !== null))
      throw new Error('Security fixture exited');
    try {
      if ((await fetch(url, { signal: AbortSignal.timeout(1000) })).ok) return;
    } catch {}
    await delay(500);
  }
  throw new Error('Security fixture startup timeout');
}
async function docker(args) {
  return new Promise((resolveCode, reject) => {
    const child = spawn('docker', args, { windowsHide: true, stdio: 'inherit' });
    child.once('error', reject);
    child.once('exit', (code) => resolveCode(code));
  });
}
try {
  await start(
    resolve(`target/debug/examples/browser_fixture${process.platform === 'win32' ? '.exe' : ''}`),
    [],
    'backend',
  );
  await waitURL('http://127.0.0.1:18092/healthz');
  await start(
    process.execPath,
    [
      resolve('web/node_modules/vite/bin/vite.js'),
      'preview',
      '--config',
      'vite.e2e.config.ts',
      '--host',
      '0.0.0.0',
    ],
    'web',
    {
      ...process.env,
      __VITE_ADDITIONAL_SERVER_ALLOWED_HOSTS: 'host.docker.internal',
    },
    resolve('web'),
  );
  await waitURL('http://127.0.0.1:4173/');
  const code = await docker([
    'run',
    '--rm',
    '--name',
    container,
    '--add-host',
    'host.docker.internal:host-gateway',
    '--mount',
    `type=bind,source=${output},target=/zap/wrk`,
    image,
    'zap-baseline.py',
    '-t',
    'http://host.docker.internal:4173/',
    '-m',
    '1',
    '-T',
    '3',
    '-J',
    'report.json',
    '-r',
    'report.html',
    '-I',
  ]);
  if (code !== 0 && code !== 2) throw new Error(`ZAP failed (${code})`);
  const report = JSON.parse(await readFile(resolve(output, 'report.json'), 'utf8'));
  const alerts = (report.site ?? []).flatMap((site) => site.alerts ?? []);
  const high = alerts.filter((alert) => Number(alert.riskcode) >= 3);
  console.log(
    `ZAP passive baseline: ${alerts.length} alert types, ${high.length} high or critical.`,
  );
  if (high.length) throw new Error('High-risk ZAP findings require correction');
} finally {
  try {
    await fetch('http://127.0.0.1:18092/__shutdown', {
      method: 'POST',
      signal: AbortSignal.timeout(3000),
    });
  } catch {}
  for (const child of children.slice().reverse()) {
    if (child.exitCode === null) {
      if (child === children[0])
        await Promise.race([new Promise((r) => child.once('exit', r)), delay(4000)]);
      if (child.exitCode === null) child.kill();
    }
  }
  for (const file of logs) await file.close();
}
